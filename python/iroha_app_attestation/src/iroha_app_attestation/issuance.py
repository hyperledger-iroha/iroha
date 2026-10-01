"""Fail-closed app-certificate signing boundary and durable one-result register.

Only a caller that has independently authenticated the release policy, issuer
preparation, raw platform evidence, distribution evidence, and applicable
revocation state may call ``DurableCertificateStore.issue_once``. A separate
provider must compose the Apple receipt or Android revocation checks before
signing; later Apple assertion counters belong to command admission. This
module exposes no route from untrusted attestation bytes to signing.
"""

from __future__ import annotations

import hashlib
import os
import sqlite3
import stat
import subprocess
import tempfile
from contextlib import closing
from dataclasses import dataclass, replace
from pathlib import Path
from typing import Callable

from .attestation import (
    AttestationRejected, RawPlatformProof, Selection, device_key_reference,
    fixed32, require, verify_issuer_preparation,
)


SIGNING_REQUEST_MAGIC = b"KAEA\x01"
SIGNING_REQUEST_BYTES = 4 + 1 + 10 * 32 + 8 + 8 + 32
MAX_CERTIFICATE_BYTES = 4096
MAX_AUDIT_EVIDENCE_BYTES = 128 * 1024
MAX_ASSERTION_LIFETIME_MS = 120_000
MAX_ENCODER_BYTES = 512 * 1024 * 1024


@dataclass(frozen=True)
class CertificateFields:
    """Exact signer input after independent policy and raw-evidence checks."""

    client_nonce: bytes
    server_nonce: bytes
    app_signing_identity_digest: bytes
    app_release_digest: bytes
    platform_evidence_digest: bytes
    release_id: bytes
    hardware_profile_id: bytes
    device_key_reference: bytes
    attested_key_id: bytes
    lane_id: bytes
    issued_at_ms: int
    expires_at_ms: int

    def request(self, authority_public_key: bytes) -> bytes:
        """Build the bounded versioned local request for the Iroha-owned encoder."""
        fields = tuple(fixed32(getattr(self, name), name) for name in (
            "client_nonce", "server_nonce", "app_signing_identity_digest",
            "app_release_digest", "platform_evidence_digest", "release_id",
            "hardware_profile_id", "device_key_reference", "attested_key_id",
            "lane_id",
        ))
        require(all(any(field) for field in fields) and fields[0] != fields[1],
                "invalid app-enrollment assertion field")
        require(0 < self.issued_at_ms < self.expires_at_ms <= (1 << 64) - 1
                and self.expires_at_ms - self.issued_at_ms <= MAX_ASSERTION_LIFETIME_MS,
                "invalid app-enrollment assertion lifetime")
        request = (SIGNING_REQUEST_MAGIC + b"".join(fields)
                   + self.issued_at_ms.to_bytes(8, "little")
                   + self.expires_at_ms.to_bytes(8, "little")
                   + fixed32(authority_public_key, "authority public key"))
        require(any(authority_public_key), "invalid app-enrollment authority key")
        require(len(request) == SIGNING_REQUEST_BYTES, "invalid certificate signing request")
        return request

    def matches_selection(self, selected: Selection, derived_key_reference: bytes) -> None:
        """Bind the raw attested point to every signed issuer-preparation field."""
        selected.transcript()
        require(self.client_nonce == selected.client_nonce
                and self.server_nonce == selected.server_nonce
                and self.release_id == selected.release_id
                and self.hardware_profile_id == selected.hardware_profile_id
                and self.device_key_reference == fixed32(derived_key_reference, "device key")
                and self.lane_id == selected.lane_id,
                "certificate differs from prepared platform and device selection")


@dataclass(frozen=True)
class GovernedIssuanceScope:
    """Release/issuer selections supplied from trusted config and checked raw proof.

    Construction does not authenticate its inputs. The production caller must
    obtain this scope from authenticated release state and the raw verifier,
    then satisfy independent distribution and revocation checks before calling
    the store. ``provider.py`` composes these checks from injected policy; its
    deployment still needs authenticated configuration and signing custody.
    """

    selection: Selection
    account_canonical: str
    issuer_policy_id: bytes
    issuer_public_spki_der: bytes
    issuer_public_spki_sha256: bytes
    trusted_time_ms: int
    openssl_path: Path
    platform: str
    raw_platform_proof: RawPlatformProof
    governed_release_id: bytes
    governed_hardware_profile_id: bytes
    governed_lane_id: bytes
    governed_app_signing_digest: bytes
    governed_app_release_digest: bytes
    governed_max_lifetime_ms: int
    authority_public_key: bytes
    fields: CertificateFields

    def verified_request(self, signed_preparation: bytes, *, fresh: bool) -> bytes:
        """Authenticate preparation and bind its six fields to the raw key."""
        preparation = verify_issuer_preparation(
            signed_preparation, self.selection, self.account_canonical,
            self.issuer_policy_id, self.issuer_public_spki_der,
            self.issuer_public_spki_sha256, self.trusted_time_ms,
            self.openssl_path, require_fresh=fresh,
        )
        proof = self.raw_platform_proof
        require(self.platform in ("apple_app_attest", "android_keymint")
                and proof.platform == self.platform,
                "platform differs from governed profile")
        require(self.selection.release_id == fixed32(self.governed_release_id, "governed release")
                and self.selection.hardware_profile_id
                == fixed32(self.governed_hardware_profile_id, "governed profile")
                and self.selection.lane_id == fixed32(self.governed_lane_id, "governed lane"),
                "prepared release, profile or lane differs from governance")
        point = proof.attested_public_key_sec1
        require(proof.device_key_reference == device_key_reference(point)
                and self.fields.platform_evidence_digest == proof.evidence_sha256
                and self.fields.attested_key_id == hashlib.sha256(point).digest(),
                "raw platform key or evidence differs from signed assertion")
        if self.platform == "apple_app_attest":
            require(self.selection.attested_key_id == hashlib.sha256(point).digest(),
                    "prepared Apple key ID differs from attested key")
        else:
            require(self.selection.attested_key_id == b"\0" * 32,
                    "Android key-generation challenge differs from governed profile")
        self.fields.matches_selection(self.selection, proof.device_key_reference)
        require(self.fields.app_signing_identity_digest
                == fixed32(self.governed_app_signing_digest, "governed app identity")
                and self.fields.app_release_digest
                == fixed32(self.governed_app_release_digest, "governed app release")
                and self.fields.issued_at_ms == preparation.issued_at_ms
                and self.fields.expires_at_ms <= preparation.expires_at_ms
                and 0 < self.governed_max_lifetime_ms <= MAX_ASSERTION_LIFETIME_MS
                and self.fields.expires_at_ms - self.fields.issued_at_ms
                <= self.governed_max_lifetime_ms
                and self.fields.issued_at_ms <= self.trusted_time_ms
                and (not fresh or self.trusted_time_ms < self.fields.expires_at_ms),
                "certificate scope or issuance time differs from governed preparation")
        return self.fields.request(self.authority_public_key)


def encode_with_iroha(
    request: bytes, encoder: Path, encoder_sha256: bytes, authority_key_fd: int,
) -> bytes:
    """Sign via an exact-content-pinned binary linked to Iroha's Norito model.

    ``authority_key_fd`` is an inherited descriptor containing an exact 32-byte
    Ed25519 seed. It is neither passed through argv nor persisted by this module.
    The caller owns its lifetime and must arrange secure custody.
    """
    require(len(request) == SIGNING_REQUEST_BYTES and request.startswith(SIGNING_REQUEST_MAGIC),
            "invalid certificate signing request")
    return _encode_with_iroha_raw(request, encoder, encoder_sha256, authority_key_fd,
                                  maximum_certificate_bytes=MAX_CERTIFICATE_BYTES)


def encode_ordinary_with_iroha(request: bytes, encoder: Path, encoder_sha256: bytes,
                              authority_key_fd: int) -> bytes:
    """Invoke the actual model encoder for the sole first-release KOAC input.

    Signer custody remains the authenticated deployment's responsibility. This
    boundary accepts no decoded certificate, arbitrary codec or signer callback.
    """
    from .ordinary_enrollment import SIGNING_REQUEST_BYTES as ordinary_bytes, SIGNING_REQUEST_MAGIC as ordinary_magic
    require(type(request) is bytes and len(request) == ordinary_bytes
            and request.startswith(ordinary_magic), 'invalid ordinary signing request')
    return _encode_with_iroha_raw(request, encoder, encoder_sha256, authority_key_fd,
                                  maximum_certificate_bytes=16 * 1024)


def encode_ordinary_with_iroha_fd(request: bytes, encoder_fd: int, encoder_sha256: bytes,
                                 authority_key_fd: int) -> bytes:
    """Execute the exact retained encoder descriptor selected by Native startup."""
    from .ordinary_enrollment import SIGNING_REQUEST_BYTES as width, SIGNING_REQUEST_MAGIC as magic
    require(type(request) is bytes and len(request) == width and request.startswith(magic),
            "invalid ordinary signing request")
    return _encode_with_iroha_raw(request, None, encoder_sha256, authority_key_fd,
        maximum_certificate_bytes=16*1024, encoder_fd=encoder_fd)


def encode_refresh_with_iroha_fd(request: bytes, encoder_fd: int, encoder_sha256: bytes,
                                authority_key_fd: int) -> bytes:
    """Canonical Native KRPI encoder only; no Python Norito lease encoding."""
    require(type(request) is bytes and 449 <= len(request) <= 513 and request[:5] == b"KRPI\x01"
            and len(request) == 409+int.from_bytes(request[407:409],"little")+32,
            "invalid Integrity lease signing request")
    return _encode_with_iroha_raw(request,None,encoder_sha256,authority_key_fd,
        maximum_certificate_bytes=4096,encoder_fd=encoder_fd)


def encode_refresh_with_iroha(request: bytes, encoder: Path, encoder_sha256: bytes,
                            authority_key_fd: int) -> bytes:
    """Same canonical KRPI purpose through an original-content-pinned encoder."""
    require(type(request) is bytes and 449 <= len(request) <= 513 and request[:5] == b"KRPI\x01"
            and len(request) == 409+int.from_bytes(request[407:409],"little")+32,
            "invalid Integrity lease signing request")
    return _encode_with_iroha_raw(request,encoder,encoder_sha256,authority_key_fd,
        maximum_certificate_bytes=4096)


def encode_raw_admission_with_iroha(request: bytes, encoder: Path, encoder_sha256: bytes,
                                   authority_key_fd: int) -> bytes:
    """Invoke only the sole KRAC01 model encoder using held signer custody."""
    from .ordinary_raw_admission import REQUEST_BYTES, REQUEST_MAGIC, TRANSPORT_BYTES
    require(type(request) is bytes and len(request) == REQUEST_BYTES
            and request.startswith(REQUEST_MAGIC), "invalid raw admission signing request")
    original = _encode_with_iroha_raw(request, encoder, encoder_sha256, authority_key_fd,
                                    maximum_certificate_bytes=TRANSPORT_BYTES)
    require(len(original) == TRANSPORT_BYTES, "raw admission encoder transport width differs")
    return original



def encode_raw_admission_with_iroha_fd(request: bytes, encoder_fd: int, encoder_sha256: bytes,
                                       authority_key_fd: int) -> bytes:
    """Execute the dedicated retained raw-admission child for sole KRAC01 input."""
    from .ordinary_raw_admission import REQUEST_BYTES, REQUEST_MAGIC
    require(type(request) is bytes and len(request) == REQUEST_BYTES
            and request.startswith(REQUEST_MAGIC), "invalid raw-admission signing request")
    result = _encode_with_iroha_raw(request, None, encoder_sha256, authority_key_fd,
        maximum_certificate_bytes=314, encoder_fd=encoder_fd)
    require(len(result) == 314, "invalid raw-admission signed transport")
    return result


def _encode_with_iroha_raw(request: bytes, encoder: Path | None, encoder_sha256: bytes,
                           authority_key_fd: int, *, maximum_certificate_bytes: int,
                           encoder_fd: int | None = None) -> bytes:
    require(isinstance(authority_key_fd, int) and authority_key_fd >= 3,
            "invalid signer key descriptor")
    require((encoder_fd is None and isinstance(encoder,Path) and encoder.is_absolute()
             and not encoder.is_symlink() and encoder.is_file())
            or (encoder is None and type(encoder_fd) is int and encoder_fd >= 3),
            "invalid Iroha certificate encoder custody")
    expected_digest = fixed32(encoder_sha256, "encoder executable pin")
    # Execute the exact bytes checked below. Hashing the configured path and later
    # executing that path would let a replacement binary inherit the signing key fd.
    with tempfile.TemporaryDirectory(prefix="kagemusha-encoder-") as temporary:
        private_copy = Path(temporary) / "verified-encoder"
        source_fd = os.open(encoder, os.O_RDONLY | os.O_NOFOLLOW) if encoder_fd is None else os.dup(encoder_fd)
        try:
            source = os.fstat(source_fd)
            require(stat.S_ISREG(source.st_mode)
                    and 0 < source.st_size <= MAX_ENCODER_BYTES,
                    "Iroha certificate encoder outside executable bound")
            output_fd = os.open(private_copy,
                                os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
            try:
                digest = hashlib.sha256()
                copied = 0
                while True:
                    chunk = os.pread(source_fd, 1024 * 1024, copied)
                    if not chunk:
                        break
                    copied += len(chunk)
                    require(copied <= MAX_ENCODER_BYTES,
                            "Iroha certificate encoder outside executable bound")
                    digest.update(chunk)
                    offset = 0
                    while offset < len(chunk):
                        offset += os.write(output_fd, chunk[offset:])
                require(copied == source.st_size and digest.digest() == expected_digest,
                        "Iroha certificate encoder differs from pinned binary")
                after = os.fstat(source_fd)
                require((source.st_dev,source.st_ino,source.st_size,source.st_mode,source.st_uid,
                         source.st_mtime_ns,source.st_ctime_ns)
                        == (after.st_dev,after.st_ino,after.st_size,after.st_mode,after.st_uid,
                            after.st_mtime_ns,after.st_ctime_ns),
                        "Iroha certificate encoder original changed")
                os.fsync(output_fd)
                os.fchmod(output_fd, 0o700)
            finally:
                os.close(output_fd)
        finally:
            os.close(source_fd)
        result = subprocess.run(
            [str(private_copy), "--key-fd", str(authority_key_fd)], input=request,
            capture_output=True, check=False, timeout=10, pass_fds=(authority_key_fd,),
            env={"PATH": "/usr/bin:/bin"},
        )
    require(result.returncode == 0 and 0 < len(result.stdout) <= maximum_certificate_bytes,
            "Iroha canonical certificate encoding failed")
    return result.stdout


class DurableCertificateStore:
    """Single-host SQLite register retaining one original result per signed preparation.

    The directory must be an owner-only local filesystem. The transaction keeps
    the deterministic Ed25519 signer inside its exclusive lock; a crash before
    commit may rerun signing but cannot publish a second, different result for
    the same preparation. Multi-host issuance requires a shared transactional
    store with the same uniqueness and durability semantics.
    """

    def __init__(self, path: Path) -> None:
        directory = path.parent
        mode = directory.stat()
        require(path.is_absolute() and not directory.is_symlink() and directory.is_dir()
                and stat.S_ISDIR(mode.st_mode) and mode.st_uid == os.getuid()
                and mode.st_mode & 0o077 == 0,
                "certificate store directory must be owner-only")
        if path.is_symlink():
            raise AttestationRejected("certificate store cannot be a symlink")
        if path.exists():
            existing = path.stat()
            require(stat.S_ISREG(existing.st_mode) and existing.st_uid == os.getuid()
                    and existing.st_mode & 0o077 == 0,
                    "certificate store must be an owner-only regular file")
        else:
            descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
            os.close(descriptor)
        self.path = path
        with closing(self._connect()) as connection:
            connection.execute("""CREATE TABLE IF NOT EXISTS app_certificates (
                preparation_sha256 BLOB PRIMARY KEY NOT NULL,
                request_sha256 BLOB NOT NULL,
                evidence_sha256 BLOB NOT NULL,
                evidence BLOB NOT NULL,
                certificate BLOB NOT NULL,
                certificate_sha256 BLOB NOT NULL,
                device_key_reference BLOB NOT NULL UNIQUE
            )""")

    def _connect(self) -> sqlite3.Connection:
        connection = sqlite3.connect(self.path, isolation_level=None, timeout=30)
        connection.execute("PRAGMA journal_mode=DELETE")
        connection.execute("PRAGMA synchronous=FULL")
        return connection

    def _has_preparation(self, signed_preparation: bytes) -> bool:
        with closing(self._connect()) as connection:
            return connection.execute(
                "SELECT 1 FROM app_certificates WHERE preparation_sha256 = ?",
                (hashlib.sha256(signed_preparation).digest(),),
            ).fetchone() is not None

    def issue_once(
        self, scope: GovernedIssuanceScope, signed_preparation: bytes,
        audit_evidence: bytes, signer: Callable[[bytes], bytes],
        refresh_scope: Callable[[], GovernedIssuanceScope],
    ) -> bytes:
        """Authenticate the issuer token and publish one certificate.

        A previously committed identical result can be recovered after the
        preparation TTL. New issuance rechecks the authenticated provider scope
        under the SQLite write lock immediately before invoking the signer.
        ``refresh_scope`` must obtain a new trusted time and verify the same
        governed policy and raw platform evidence.
        """
        require(type(scope) is GovernedIssuanceScope,
                "authenticated issuance scope is required")
        request = scope.verified_request(signed_preparation, fresh=False)
        require(hashlib.sha256(audit_evidence).digest()
                == scope.raw_platform_proof.evidence_sha256,
                "audit evidence differs from checked raw platform evidence")
        require(callable(refresh_scope), "fresh issuance scope is required")
        if not self._has_preparation(signed_preparation):
            scope.verified_request(signed_preparation, fresh=True)

        def revalidate_new_issuance() -> None:
            # A competing writer may hold BEGIN IMMEDIATE past the preparation
            # deadline. Recheck the trusted provider inside our write lock, just
            # before invoking the signer, with no policy or evidence substitution.
            latest = refresh_scope()
            require(type(latest) is GovernedIssuanceScope
                    and latest.trusted_time_ms >= scope.trusted_time_ms
                    and replace(latest, trusted_time_ms=scope.trusted_time_ms) == scope,
                    "issuance scope changed before signing")
            require(latest.verified_request(signed_preparation, fresh=True) == request,
                    "issuer preparation expired before signing")

        return self._issue_once(signed_preparation, request, audit_evidence, signer,
                                revalidate_new_issuance)

    def recover(
        self, scope: GovernedIssuanceScope, signed_preparation: bytes,
        audit_evidence: bytes,
    ) -> bytes:
        """Return only an existing exact result; never call or reserve the signer."""
        request = scope.verified_request(signed_preparation, fresh=False)
        require(hashlib.sha256(audit_evidence).digest()
                == scope.raw_platform_proof.evidence_sha256,
                "audit evidence differs from checked raw platform evidence")
        with closing(self._connect()) as connection:
            current = connection.execute(
                "SELECT request_sha256, evidence_sha256, evidence, certificate, certificate_sha256, "
                "device_key_reference "
                "FROM app_certificates WHERE preparation_sha256 = ?",
                (hashlib.sha256(signed_preparation).digest(),),
            ).fetchone()
        require(current is not None, "app-certificate result is not available for recovery")
        require(current[0] == hashlib.sha256(request).digest()
                and current[1] == hashlib.sha256(audit_evidence).digest()
                and current[2] == audit_evidence
                and hashlib.sha256(current[3]).digest() == current[4]
                and current[5] == scope.fields.device_key_reference,
                "conflicting or corrupt app-certificate recovery")
        return current[3]

    def _issue_once(
        self, signed_preparation: bytes, signing_request: bytes, audit_evidence: bytes,
        signer: Callable[[bytes], bytes],
        revalidate_new_issuance: Callable[[], None] | None = None,
    ) -> bytes:
        """Atomically retain the exact first result or recover it on identical retry.

        ``signed_preparation`` and all policy/evidence gates must already have
        passed before calling this method. Conflicting retries never invoke the
        signer. A retry can recover after preparation expiry if the authenticated
        caller has separately verified the original signature and identity.
        Public issuance supplies ``revalidate_new_issuance``; direct internal
        calls without it are only suitable for isolated persistence tests.
        """
        require(len(signed_preparation) == 273 and signed_preparation[0] == 1,
                "invalid signed preparation frame")
        require(len(signing_request) == SIGNING_REQUEST_BYTES
                and signing_request.startswith(SIGNING_REQUEST_MAGIC),
                "invalid canonical signing request")
        require(0 < len(audit_evidence) <= MAX_AUDIT_EVIDENCE_BYTES,
                "audit evidence outside bound")
        token_hash = hashlib.sha256(signed_preparation).digest()
        request_hash = hashlib.sha256(signing_request).digest()
        evidence_hash = hashlib.sha256(audit_evidence).digest()
        # The eighth fixed 32-byte request field is the attested device key.
        # One hardware key must not enroll a second monetary lane or account.
        device_key_reference = signing_request[5 + 7 * 32:5 + 8 * 32]
        require(len(device_key_reference) == 32 and device_key_reference != bytes(32),
                "invalid app-certificate device key reference")
        with closing(self._connect()) as connection:
            connection.execute("BEGIN IMMEDIATE")
            current = connection.execute(
                "SELECT request_sha256, evidence_sha256, evidence, certificate, certificate_sha256, "
                "device_key_reference "
                "FROM app_certificates WHERE preparation_sha256 = ?", (token_hash,),
            ).fetchone()
            if current is not None:
                require(current[0] == request_hash and current[1] == evidence_hash
                        and current[2] == audit_evidence
                        and hashlib.sha256(current[3]).digest() == current[4]
                        and current[5] == device_key_reference,
                        "conflicting or corrupt app-certificate recovery")
                connection.commit()
                return current[3]
            require(connection.execute(
                "SELECT 1 FROM app_certificates WHERE device_key_reference = ?",
                (device_key_reference,),
            ).fetchone() is None, "attested device key already enrolled")
            if revalidate_new_issuance is not None:
                revalidate_new_issuance()
            certificate = signer(signing_request)
            require(isinstance(certificate, bytes)
                    and 0 < len(certificate) <= MAX_CERTIFICATE_BYTES,
                    "canonical certificate signer returned an invalid frame")
            connection.execute(
                "INSERT INTO app_certificates VALUES (?, ?, ?, ?, ?, ?, ?)",
                (token_hash, request_hash, evidence_hash, audit_evidence, certificate,
                 hashlib.sha256(certificate).digest(), device_key_reference),
            )
            connection.commit()
            return certificate
