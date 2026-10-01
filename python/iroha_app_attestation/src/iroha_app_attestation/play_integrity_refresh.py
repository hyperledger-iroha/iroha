"""Exact model-owned periodic Play Integrity challenge and lease transports.

These codecs confer no issuer or financial authority. The deployment-owned
store must select its unchanged, previously issued canonical credential and
current Native policy before invoking the real Google decoder or signer.
"""
from __future__ import annotations

import hashlib
import subprocess
import tempfile
from dataclasses import dataclass
from pathlib import Path

from .attestation import (children, der_one, fixed32, positive_integer,
                          public_key_pem, require)
from .ordinary_provider import OrdinaryReleasePolicy
from .play_integrity import PlayIntegrityProof

CHALLENGE_DOMAIN = b"iroha:kagemusha:v1:play-integrity-refresh-challenge\0"
REQUEST_DOMAIN = b"iroha:kagemusha:v1:play-integrity-refresh-request\0"
POSSESSION_DOMAIN = b"iroha:kagemusha:v1:play-integrity-refresh-possession\0"
LEASE_DOMAIN = b"iroha:kagemusha:v1:play-integrity-refresh-lease\0"
CREDENTIAL_DOMAIN = b"iroha:kagemusha:v1:ordinary-app-credential-original\0"
CHALLENGE_BODY_BYTES = 450
CHALLENGE_TRANSPORT_BYTES = 514
LEASE_BODY_BYTES = 402
SIGNING_REQUEST_MAGIC = b"KRPI\x01"
FIELDS = ("credential_digest", "attested_key_id", "account_binding", "network_id", "lane_id",
          "release_id", "hardware_profile_id", "suite_id", "trust_policy_digest",
          "app_authority_policy_digest", "play_integrity_policy_digest", "nonce",
          "original_enrollment_challenge_digest")


def _message(domain: bytes, body: bytes) -> bytes:
    return domain + len(body).to_bytes(8, "little") + body


@dataclass(frozen=True)
class PlayIntegrityRefreshChallenge:
    credential_digest: bytes
    attested_key_id: bytes
    account_binding: bytes
    network_id: bytes
    lane_id: bytes
    release_id: bytes
    hardware_profile_id: bytes
    suite_id: bytes
    trust_policy_digest: bytes
    app_authority_policy_digest: bytes
    play_integrity_policy_digest: bytes
    nonce: bytes
    original_enrollment_challenge_digest: bytes
    policy_epoch: int
    hardware_epoch: int
    issued_at_ms: int
    expires_at_ms: int

    def signing_bytes(self) -> bytes:
        selectors = tuple(fixed32(getattr(self, field), field) for field in FIELDS)
        times = (self.policy_epoch, self.hardware_epoch, self.issued_at_ms, self.expires_at_ms)
        require(all(any(value) for value in selectors)
                and all(type(value) is int and 0 < value < (1 << 64) for value in times)
                and self.issued_at_ms < self.expires_at_ms
                and self.expires_at_ms - self.issued_at_ms <= 120_000,
                "invalid original Play Integrity refresh challenge")
        body = b"\x01\x00" + b"".join(selectors) + b"".join(value.to_bytes(8, "little") for value in times)
        require(len(body) == CHALLENGE_BODY_BYTES, "refresh challenge layout differs")
        return _message(CHALLENGE_DOMAIN, body)

    def attempt_id(self) -> bytes:
        return hashlib.sha256(self.signing_bytes()).digest()

    def request_hash(self) -> bytes:
        return hashlib.sha256(REQUEST_DOMAIN + self.signing_bytes() + self.attested_key_id).digest()

    def possession_message(self) -> bytes:
        return _message(POSSESSION_DOMAIN, self.signing_bytes())

    def select_issued_original(self, *, signing_request: bytes, certificate: bytes,
                              policy: OrdinaryReleasePolicy, now_ms: int, fresh: bool) -> bytes:
        """Match only originals retained by the authenticated issuance register.

        The fixed KOAC body is an encoder transport, not a Python Norito codec.
        The caller must obtain it and the complete certificate from its durable
        issuer register. Publicly supplied copies cannot create that custody.
        """
        self.signing_bytes(); policy.validate()
        require(type(signing_request) is bytes and len(signing_request) == 831
                and signing_request[:5] == b"KOAC\x01"
                and type(certificate) is bytes and 0 < len(certificate) <= 16*1024
                and type(now_ms) is int and 0 < now_ms < (1 << 64) and type(fresh) is bool,
                "retained original ordinary credential absent")
        body = signing_request[5:-32]
        fields = [body[4+i*32:4+(i+1)*32] for i in range(18)]
        point = body[580:645]
        credential_issued = int.from_bytes(body[661:669], "little")
        credential_expires = int.from_bytes(body[669:677], "little")
        integrity = policy.play_integrity_policy
        require(body[:3] == b"\x01\x00\x01" and body[3] in (1, 2)
                and signing_request[-32:] == policy.authority_public_key
                and self.credential_digest == hashlib.sha256(_message(CREDENTIAL_DOMAIN, certificate)).digest()
                and self.attested_key_id == fields[13] == hashlib.sha256(point).digest()
                and all(getattr(self, name) == fields[index] for name, index in
                        (("account_binding", 3), ("network_id", 4), ("lane_id", 5), ("release_id", 6),
                         ("hardware_profile_id", 7), ("suite_id", 8), ("trust_policy_digest", 9),
                         ("app_authority_policy_digest", 10), ("original_enrollment_challenge_digest", 17)))
                and all(getattr(self, name) == getattr(policy, name) for name in
                        ("network_id", "release_id", "hardware_profile_id", "suite_id", "trust_policy_digest",
                         "app_authority_policy_digest", "policy_epoch"))
                and self.policy_epoch == int.from_bytes(body[645:653], "little")
                and self.hardware_epoch == int.from_bytes(body[653:661], "little")
                and integrity is not None and body[681] == 1
                and self.play_integrity_policy_digest == integrity.policy_digest == body[746:778]
                and policy.profile_valid_from_ms <= now_ms < policy.profile_expires_at_ms
                and credential_issued <= self.issued_at_ms < self.expires_at_ms <= credential_expires
                and self.issued_at_ms <= now_ms < credential_expires
                and (not fresh or now_ms < self.expires_at_ms),
                "refresh differs from retained credential and current Native policy")
        return point


def decode_refresh_transport(original: bytes) -> PlayIntegrityRefreshChallenge:
    require(type(original) is bytes and len(original) == CHALLENGE_TRANSPORT_BYTES
            and original[:2] == b"\x01\x00", "invalid signed refresh transport")
    body = original[:CHALLENGE_BODY_BYTES]
    fields = [body[2+i*32:2+(i+1)*32] for i in range(13)]
    times = [int.from_bytes(body[418+i*8:426+i*8], "little") for i in range(4)]
    selected = PlayIntegrityRefreshChallenge(*fields, *times)
    require(selected.signing_bytes()[-CHALLENGE_BODY_BYTES:] == body, "refresh body roundtrip differs")
    return selected


def authenticate_refresh_transport(original: bytes, *, public_key: bytes,
                                   openssl_path: Path) -> PlayIntegrityRefreshChallenge:
    selected = decode_refresh_transport(original)
    key = fixed32(public_key, "independently selected Core refresh signer")
    require(any(key) and isinstance(openssl_path, Path) and openssl_path.is_absolute()
            and openssl_path.is_file(), "refresh Core verification environment absent")
    with tempfile.TemporaryDirectory(prefix="iroha-integrity-core-") as temporary:
        directory = Path(temporary)
        (directory/"public.pem").write_bytes(public_key_pem(bytes.fromhex("302a300506032b6570032100") + key))
        (directory/"message").write_bytes(selected.signing_bytes())
        (directory/"signature").write_bytes(original[CHALLENGE_BODY_BYTES:])
        result = subprocess.run([str(openssl_path), "pkeyutl", "-verify", "-pubin", "-inkey",
            str(directory/"public.pem"), "-rawin", "-in", str(directory/"message"), "-sigfile",
            str(directory/"signature")], stdin=subprocess.DEVNULL, capture_output=True, timeout=5, check=False)
        require(result.returncode == 0, "Core refresh signature rejected")
    return selected


def verify_refresh_possession(selected: PlayIntegrityRefreshChallenge, point: bytes,
                              original_der: bytes, openssl_path: Path) -> None:
    require(type(point) is bytes and len(point) == 65 and point[0] == 4
            and hashlib.sha256(point).digest() == selected.attested_key_id
            and type(original_der) is bytes and 8 <= len(original_der) <= 72,
            "refresh possession key or signature differs")
    scalars = children(der_one(original_der))
    require(len(scalars) == 2 and all(0 < positive_integer(value) < (1 << 256) for value in scalars)
            and isinstance(openssl_path, Path) and openssl_path.is_absolute() and openssl_path.is_file(),
            "invalid refresh possession verification inputs")
    with tempfile.TemporaryDirectory(prefix="iroha-integrity-possession-") as temporary:
        directory = Path(temporary)
        spki = bytes.fromhex("3059301306072a8648ce3d020106082a8648ce3d030107034200") + point
        (directory/"public.pem").write_bytes(public_key_pem(spki))
        (directory/"message").write_bytes(selected.possession_message())
        (directory/"signature").write_bytes(original_der)
        result = subprocess.run([str(openssl_path), "dgst", "-sha256", "-verify",
            str(directory/"public.pem"), "-signature", str(directory/"signature"), str(directory/"message")],
            stdin=subprocess.DEVNULL, capture_output=True, timeout=5, check=False)
        require(result.returncode == 0, "refresh possession signature rejected")


def refresh_lease_signing_request(selected: PlayIntegrityRefreshChallenge, original_der: bytes,
                                 proof: PlayIntegrityProof, policy: OrdinaryReleasePolicy,
                                 *, verified_at_ms: int, issued_at_ms: int, expires_at_ms: int,
                                 credential_expires_at_ms: int) -> bytes:
    """Create only the model's KRPI input after actual Google and possession checks."""
    selected.signing_bytes(); policy.validate(); integrity = policy.play_integrity_policy
    require(type(proof) is PlayIntegrityProof and integrity is not None,
            "genuine governed refresh verdict absent")
    refresh_before_ms = min(verified_at_ms + integrity.maximum_refresh_interval_ms,
                            credential_expires_at_ms, policy.profile_expires_at_ms) if type(verified_at_ms) is int else 0
    require(type(original_der) is bytes and 8 <= len(original_der) <= 72
            and proof.request_hash == selected.request_hash()
            and proof.policy_digest == selected.play_integrity_policy_digest == integrity.policy_digest
            and all(type(time) is int and 0 < time < (1 << 64) for time in
                    (verified_at_ms, refresh_before_ms, issued_at_ms, expires_at_ms, credential_expires_at_ms))
            and selected.issued_at_ms <= verified_at_ms <= issued_at_ms < selected.expires_at_ms
            and selected.issued_at_ms <= proof.timestamp_ms <= verified_at_ms
            and verified_at_ms - proof.timestamp_ms <= integrity.maximum_evidence_age_ms
            and 0 < refresh_before_ms - verified_at_ms <= integrity.maximum_refresh_interval_ms
            and issued_at_ms < expires_at_ms <= min(refresh_before_ms, credential_expires_at_ms,
                                                    policy.profile_expires_at_ms),
            "refresh lease differs from genuine verdict and original interval")
    fields = (selected.credential_digest, selected.attempt_id(), selected.attested_key_id,
              selected.release_id, selected.hardware_profile_id, selected.trust_policy_digest,
              selected.app_authority_policy_digest, proof.request_hash, proof.google_response_sha256,
              proof.policy_digest, hashlib.sha256(original_der).digest())
    require(all(any(fixed32(value, "refresh lease field")) for value in fields), "empty refresh lease field")
    times = (selected.policy_epoch, selected.hardware_epoch, verified_at_ms,
             refresh_before_ms, issued_at_ms, expires_at_ms)
    body = b"\x01\x00" + b"".join(fields) + b"".join(time.to_bytes(8, "little") for time in times)
    require(len(body) == LEASE_BODY_BYTES, "refresh lease layout differs")
    return SIGNING_REQUEST_MAGIC + body + len(original_der).to_bytes(2, "little") + original_der + policy.authority_public_key
