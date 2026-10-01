"""Durable ordinary enrollment originals and the actual canonical Iroha signer.

The startup owner must authenticate Native policy and select protected encoder
and signer descriptors first. This module exposes no arbitrary signer callback,
decoded-verdict input, or route from a raw proof to signing custody.
"""
from __future__ import annotations

import hashlib
import os
import sqlite3
import stat
from contextlib import closing
from pathlib import Path

from .attestation import fixed32, require
from .issuance import (encode_ordinary_with_iroha, encode_ordinary_with_iroha_fd,
                       encode_raw_admission_with_iroha, encode_raw_admission_with_iroha_fd,
                       encode_refresh_with_iroha, encode_refresh_with_iroha_fd)
from .ordinary_provider import (GovernedOrdinaryEvidenceProvider, OrdinaryCredentialRequest,
                                VerifiedOrdinaryEvidence, VerifiedOrdinaryRawEvidence,
                                OrdinaryRawAttestationRequest)
from .ordinary_raw_admission import (signing_request as raw_signing_request,
                                     authenticate_transport as authenticate_raw_transport)

MAX_CERTIFICATE_BYTES = 16 * 1024


class _CanonicalIssuerEncoderCustody:
    """Held deployment-only encoder and independent app-authority descriptors.

    The Native startup owner selects custody. Each purpose-specific native child
    checks its Ed public key against the original request pin. Python never reads
    the seed or passes it through argv/environment.
    """
    def __init__(self, *, encoder: Path | None = None, encoder_sha256: bytes,
                 authority_key_fd: int, authority_public_key: bytes,
                 encoder_fd: int | None = None, credential_owner_uid: int | None = None) -> None:
        require(type(authority_key_fd) is int and authority_key_fd >= 3,
                "ordinary signer custody absent")
        self._fd = os.dup(authority_key_fd)
        self._encoder_fd = -1
        self._credential_owner_uid = os.getuid() if credential_owner_uid is None else credential_owner_uid
        try:
            require(type(self._credential_owner_uid) is int and self._credential_owner_uid in (0,os.getuid())
                    and ((encoder_fd is None and isinstance(encoder,Path))
                         or (encoder is None and type(encoder_fd) is int and encoder_fd >= 3)),
                    "ordinary encoder Native custody differs")
            self._identity = self._metadata()
            self._public = fixed32(authority_public_key, "ordinary authority public key")
            require(any(self._public), "ordinary signer public key absent")
            self._encoder = encoder
            if encoder_fd is not None:
                self._encoder_fd = os.dup(encoder_fd)
            self._encoder_sha = fixed32(encoder_sha256, "ordinary encoder original pin")
        except Exception:
            if self._encoder_fd >= 0:
                os.close(self._encoder_fd)
                self._encoder_fd = -1
            os.close(self._fd)
            self._fd = -1
            raise

    def _metadata(self) -> tuple:
        value = os.fstat(self._fd)
        require(stat.S_ISREG(value.st_mode) and value.st_uid == self._credential_owner_uid
                and value.st_mode & 0o077 == 0 and value.st_size == 32,
                "ordinary signer must be a protected original seed descriptor")
        return (value.st_dev, value.st_ino, value.st_size, value.st_uid, value.st_mode,
                value.st_mtime_ns, value.st_ctime_ns)

    def close(self) -> None:
        if self._encoder_fd >= 0:
            os.close(self._encoder_fd)
            self._encoder_fd = -1
        if self._fd >= 0:
            os.close(self._fd)
            self._fd = -1


class CanonicalOrdinaryCredentialEncoder(_CanonicalIssuerEncoderCustody):
    """Dedicated pinned KOAC encoder/custody; accepts only final credential input."""
    def _encode(self, evidence: VerifiedOrdinaryEvidence, request: bytes) -> bytes:
        require(type(evidence) is VerifiedOrdinaryEvidence and evidence.policy.authority_public_key == self._public
                and request[-32:] == self._public and self._metadata() == self._identity,
                "ordinary signer differs from actual selected evidence/policy")
        original = (encode_ordinary_with_iroha(request, self._encoder, self._encoder_sha, self._fd)
                    if self._encoder_fd < 0 else encode_ordinary_with_iroha_fd(
                        request, self._encoder_fd, self._encoder_sha, self._fd))
        require(self._metadata() == self._identity, "ordinary signer original changed")
        return original

    def _encode_refresh(self, evidence: VerifiedOrdinaryEvidence, request: bytes) -> bytes:
        require(type(evidence) is VerifiedOrdinaryEvidence
                and evidence.policy.authority_public_key == self._public
                and request[-32:] == self._public and self._metadata() == self._identity,
                "refresh signer differs from retained enrollment/policy")
        original = (encode_refresh_with_iroha(request,self._encoder,self._encoder_sha,self._fd)
                    if self._encoder_fd < 0 else encode_refresh_with_iroha_fd(
                        request,self._encoder_fd,self._encoder_sha,self._fd))
        require(self._metadata() == self._identity,"ordinary signer original changed")
        return original



class CanonicalRawAppAdmissionEncoder(_CanonicalIssuerEncoderCustody):
    """Dedicated pinned KRAC01 child/custody; cannot encode a final credential."""
    def _encode(self, evidence: VerifiedOrdinaryRawEvidence, request: bytes) -> bytes:
        require(type(evidence) is VerifiedOrdinaryRawEvidence
                and evidence.policy.authority_public_key == self._public
                and request == raw_signing_request(evidence)
                and request[-32:] == self._public and self._metadata() == self._identity,
                "raw signer differs from checked originals/policy")
        original = (encode_raw_admission_with_iroha(request, self._encoder, self._encoder_sha, self._fd)
                    if self._encoder_fd < 0 else encode_raw_admission_with_iroha_fd(
                        request, self._encoder_fd, self._encoder_sha, self._fd))
        require(self._metadata() == self._identity, "raw signer original changed")
        return original



class DurableOrdinaryCredentialIssuer:
    """One exact response per signed515 attempt, with staged Google originals.

    A successful Google Standard token decode is retained in its own committed
    transaction before signing; an identical retry never decodes it again.
    Signing time/input are also committed before signing, so a crashed signer
    retry uses the same original. Recovery returns only an existing certificate,
    with real current policy, chain/revocation and possession checks, and never
    reaches Google decoding or signer custody.
    """
    def __init__(self, *, path: Path, provider: GovernedOrdinaryEvidenceProvider,
                 encoder: CanonicalOrdinaryCredentialEncoder,
                 raw_encoder: CanonicalRawAppAdmissionEncoder) -> None:
        require(type(provider) is GovernedOrdinaryEvidenceProvider
                and type(encoder) is CanonicalOrdinaryCredentialEncoder
                and type(raw_encoder) is CanonicalRawAppAdmissionEncoder,
                "ordinary deployment issuer owners absent")
        require(path.is_absolute() and not path.is_symlink() and path.parent.is_dir()
                and not path.parent.is_symlink(), "ordinary issuer register path invalid")
        parent = path.parent.stat()
        require(parent.st_uid == os.getuid() and parent.st_mode & 0o077 == 0,
                "ordinary issuer register must be owner-only")
        if not path.exists():
            descriptor = os.open(path, os.O_CREAT | os.O_EXCL | os.O_WRONLY | os.O_NOFOLLOW, 0o600)
            os.close(descriptor)
        value = path.stat()
        require(stat.S_ISREG(value.st_mode) and value.st_uid == os.getuid()
                and value.st_mode & 0o077 == 0, "ordinary issuer register must be protected")
        self._path = path
        self._database_identity = (value.st_dev, value.st_ino)
        self._provider = provider
        self._encoder = encoder
        self._raw_encoder = raw_encoder
        with closing(self._connect()) as connection:
            connection.execute("""CREATE TABLE IF NOT EXISTS ordinary_app_attempts (
                operation_id BLOB PRIMARY KEY NOT NULL,
                attempt_original BLOB NOT NULL,
                app_key_reference BLOB NOT NULL UNIQUE,
                google_original BLOB,
                google_original_sha256 BLOB,
                google_verified_at_ms INTEGER,
                signing_request BLOB,
                certificate BLOB,
                certificate_sha256 BLOB
            )""")

            connection.execute("""CREATE TABLE IF NOT EXISTS ordinary_raw_attempts (
                operation_id BLOB PRIMARY KEY NOT NULL,
                attempt_original BLOB NOT NULL,
                app_key_reference BLOB NOT NULL UNIQUE,
                signing_request BLOB NOT NULL,
                admission BLOB,
                admission_sha256 BLOB
            )""")
            connection.execute("""CREATE TABLE IF NOT EXISTS ordinary_possession_originals (
                operation_id BLOB PRIMARY KEY NOT NULL,
                raw_admission_sha256 BLOB NOT NULL,
                possession_original BLOB NOT NULL,
                checked_apple_counter INTEGER NOT NULL
            )""")

    def _recheck_database(self) -> None:
        value = self._path.lstat()
        parent = self._path.parent.lstat()
        require(stat.S_ISREG(value.st_mode) and value.st_uid == os.getuid()
                and value.st_mode & 0o077 == 0 and (value.st_dev, value.st_ino) == self._database_identity
                and stat.S_ISDIR(parent.st_mode) and parent.st_uid == os.getuid()
                and parent.st_mode & 0o077 == 0, "ordinary issuer register original changed")

    def _connect(self) -> sqlite3.Connection:
        self._recheck_database()
        connection = sqlite3.connect(self._path, isolation_level=None, timeout=30)
        connection.execute("PRAGMA journal_mode=DELETE")
        connection.execute("PRAGMA synchronous=FULL")
        self._recheck_database()
        return connection

    @staticmethod
    def _lookup_raw(connection, evidence: VerifiedOrdinaryRawEvidence):
        row = connection.execute("SELECT attempt_original,app_key_reference,signing_request,"
            "admission,admission_sha256 FROM ordinary_raw_attempts WHERE operation_id=?",
            (evidence.request.operation_id,)).fetchone()
        if row is not None:
            require(row[0] == evidence.request.attempt_original()
                    and row[1] == evidence.raw_proof.device_key_reference
                    and row[2] == raw_signing_request(evidence),
                    "conflicting raw enrollment retry")
            require((row[3] is None) == (row[4] is None), "corrupt raw admission result")
            if row[3] is not None:
                require(type(row[3]) is bytes and len(row[3]) == 314
                        and hashlib.sha256(row[3]).digest() == row[4],
                        "corrupt raw admission original")
        return row

    def accept_raw(self, request: OrdinaryRawAttestationRequest) -> bytes:
        """Verify/reserve exact originals, then publish one purpose-specific result.

        The raw slot is committed before the held native child encoder runs.
        Failed signing retries reuse its exact original scope/input. Recovery
        reauthenticates current policy/platform and returns only retained bytes.
        It neither consumes E nor issues a credential or monetary capability.
        """
        require(type(request) is OrdinaryRawAttestationRequest, "raw issuer request absent")
        evidence = self._provider.prepare_raw(request, fresh=False)
        with closing(self._connect()) as connection:
            connection.execute("BEGIN IMMEDIATE")
            row = self._lookup_raw(connection, evidence)
            if row is not None and row[3] is not None:
                authenticate_raw_transport(row[3], evidence, self._provider._openssl, fresh=False)
                self._recheck_database(); connection.commit()
                return row[3]
            require(request.operation == "issue", "raw admission not available for recovery")
            evidence = self._provider.prepare_raw(request, fresh=True)
            row = self._lookup_raw(connection, evidence)
            if row is None:
                require(connection.execute("SELECT 1 FROM ordinary_raw_attempts WHERE app_key_reference=?",
                    (evidence.raw_proof.device_key_reference,)).fetchone() is None,
                    "raw key already belongs to another enrollment")
                connection.execute("INSERT INTO ordinary_raw_attempts(operation_id,attempt_original,"
                    "app_key_reference,signing_request) VALUES(?,?,?,?)", (request.operation_id,
                    request.attempt_original(), evidence.raw_proof.device_key_reference, raw_signing_request(evidence)))
            self._recheck_database(); connection.commit()
        with closing(self._connect()) as connection:
            connection.execute("BEGIN IMMEDIATE")
            evidence = self._provider.prepare_raw(request, fresh=True)
            row = self._lookup_raw(connection, evidence)
            require(row is not None, "raw reservation missing")
            if row[3] is None:
                original = self._raw_encoder._encode(evidence, row[2])
                latest = self._provider.prepare_raw(request, fresh=True)
                require(latest.trusted_time_ms >= evidence.trusted_time_ms,
                        "raw trusted time regressed before publication")
                require(self._lookup_raw(connection, latest)[2] == row[2],
                        "raw original changed before publication")
                authenticate_raw_transport(original, latest, self._provider._openssl, fresh=True)
                changed = connection.execute("UPDATE ordinary_raw_attempts SET admission=?,admission_sha256=? "
                    "WHERE operation_id=? AND admission IS NULL", (original, hashlib.sha256(original).digest(),
                                                                  request.operation_id)).rowcount
                require(changed == 1, "raw publication conflict")
            else:
                original = row[3]
                authenticate_raw_transport(original, evidence, self._provider._openssl, fresh=True)
            self._recheck_database(); connection.commit()
            return original

    def _retained_raw(self, connection, evidence: VerifiedOrdinaryEvidence) -> bytes:
        raw = self._provider.prepare_raw(evidence.request.raw_request(), fresh=False)
        row = self._lookup_raw(connection, raw)
        require(row is not None and row[3] is not None,
                "independent raw admission must precede enrollment possession")
        authenticate_raw_transport(row[3], raw, self._provider._openssl, fresh=False)
        return hashlib.sha256(row[3]).digest()

    @staticmethod
    def _possession_original(evidence: VerifiedOrdinaryEvidence) -> bytes:
        e = evidence.challenge.possession_message(evidence.possession.attested_key_id,
            evidence.raw_proof.evidence_sha256, evidence.challenge.issued_at_ms,
            evidence.challenge.expires_at_ms)
        return b"iroha:kagemusha:v1:issuer-retained-enrollment-possession\0" + b"".join(
            len(original).to_bytes(8, "little") + original for original in
            (e, evidence.request.raw_possession))

    def _retain_or_check_possession(self, connection, evidence: VerifiedOrdinaryEvidence, *, reserve: bool) -> None:
        raw_digest = self._retained_raw(connection, evidence)
        original = self._possession_original(evidence)
        counter = evidence.possession.app_attest_counter_floor
        row = connection.execute("SELECT raw_admission_sha256,possession_original,checked_apple_counter "
            "FROM ordinary_possession_originals WHERE operation_id=?", (evidence.request.operation_id,)).fetchone()
        expected = (raw_digest, original, counter)
        if row is None:
            require(reserve, "retained enrollment possession absent")
            connection.execute("INSERT INTO ordinary_possession_originals(operation_id,raw_admission_sha256,"
                "possession_original,checked_apple_counter) VALUES(?,?,?,?)",
                (evidence.request.operation_id, *expected))
        else:
            require(tuple(row) == expected, "conflicting retained enrollment possession")

    @staticmethod
    def _lookup(connection, evidence: VerifiedOrdinaryEvidence):
        request = evidence.request
        row = connection.execute("SELECT attempt_original,app_key_reference,google_original,"
            "google_original_sha256,google_verified_at_ms,signing_request,certificate,certificate_sha256 "
            "FROM ordinary_app_attempts WHERE operation_id=?", (request.operation_id,)).fetchone()
        if row is not None:
            require(row[0] == request.attempt_original()
                    and row[1] == evidence.raw_proof.device_key_reference,
                    "conflicting ordinary enrollment retry")
            require((row[2] is None) == (row[3] is None) == (row[4] is None),
                    "corrupt ordinary Google result")
            if row[2] is not None:
                require(type(row[2]) is bytes and hashlib.sha256(row[2]).digest() == row[3],
                        "corrupt ordinary Google original")
            require((row[6] is None) == (row[7] is None), "corrupt ordinary certificate result")
            if row[6] is not None:
                require(type(row[6]) is bytes and 0 < len(row[6]) <= MAX_CERTIFICATE_BYTES
                        and hashlib.sha256(row[6]).digest() == row[7] and row[5] is not None,
                        "corrupt ordinary certificate original")
        return row

    def _checked_saved_input(self, connection, evidence: VerifiedOrdinaryEvidence, row, *, fresh: bool) -> bytes:
        self._retain_or_check_possession(connection, evidence, reserve=False)
        request = row[5]
        require(type(request) is bytes and len(request) == 831 and request[:5] == b"KOAC\x01",
                "corrupt ordinary signing input")
        # This is the model-owned fixed794 transport, not a second Norito codec.
        # Header4 + selectors576 + actual SEC1 point65 + two scope epochs16.
        issued = int.from_bytes(request[5+661:5+669], "little")
        expires = int.from_bytes(request[5+669:5+677], "little")
        integrity = self._provider.retained_integrity(evidence, row[2],
            verified_at_ms=row[4], fresh=fresh)
        require(evidence.signing_request(issued, expires, integrity) == request,
                "ordinary signing input differs from current verified originals")
        return request

    def _retained_refresh_enrollment(self, selected, *, fresh: bool):
        """Select genuine issued originals from this held register, never caller copies."""
        from .play_integrity_refresh import PlayIntegrityRefreshChallenge
        require(type(selected) is PlayIntegrityRefreshChallenge,"refresh challenge absent")
        self._provider._recheck()
        with closing(self._connect()) as connection:
            row=connection.execute("SELECT attempt_original FROM ordinary_app_attempts WHERE operation_id=?",
                (selected.original_enrollment_challenge_digest,)).fetchone()
            require(row is not None,"refresh requires an actual retained enrollment")
            raw=row[0];domain=b"iroha:kagemusha:v1:ordinary-app-issuer-attempt\0"
            require(type(raw) is bytes and raw.startswith(domain),"retained enrollment framing differs")
            offset=len(domain);fields=[]
            for bound in (515,65,128*1024,4096,32,64*1024):
                require(offset+8 <= len(raw),"retained enrollment framing truncated")
                length=int.from_bytes(raw[offset:offset+8],'little');offset+=8
                require(length <= bound and offset+length <= len(raw),"retained enrollment field outside bound")
                fields.append(raw[offset:offset+length]);offset+=length
            require(offset==len(raw),"retained enrollment trailing fields")
            request=OrdinaryCredentialRequest('recover',selected.original_enrollment_challenge_digest,
                *fields[:4],fields[4].decode('ascii'),fields[5].decode('ascii') if fields[5] else None)
            require(request.attempt_original()==raw,"retained enrollment original differs")
            evidence=self._provider.prepare(request,fresh=False)
            issued=self._lookup(connection,evidence)
            require(issued is not None and issued[6] is not None,"refresh requires an issued original credential")
            signing=self._checked_saved_input(connection,evidence,issued,fresh=False)
            point=selected.select_issued_original(signing_request=signing,certificate=issued[6],
                policy=evidence.policy,now_ms=evidence.trusted_time_ms,fresh=fresh)
            self._recheck_database()
            return evidence,signing,issued[6],point

    def issue(self, request: OrdinaryCredentialRequest) -> bytes:
        require(type(request) is OrdinaryCredentialRequest, "ordinary issuer request absent")
        evidence = self._provider.prepare(request, fresh=False)
        with closing(self._connect()) as connection:
            connection.execute("BEGIN IMMEDIATE")
            row = self._lookup(connection, evidence)
            if row is not None and row[6] is not None:
                self._checked_saved_input(connection, evidence, row, fresh=False)
                self._recheck_database()
                connection.commit()
                return row[6]
            require(request.operation == "issue", "ordinary credential is not available for recovery")
            evidence = self._provider.prepare(request, fresh=True)
            row = self._lookup(connection, evidence)
            if row is None:
                self._retain_or_check_possession(connection, evidence, reserve=True)
                require(connection.execute("SELECT 1 FROM ordinary_app_attempts WHERE app_key_reference=?",
                    (evidence.raw_proof.device_key_reference,)).fetchone() is None,
                    "ordinary approval key already belongs to another enrollment attempt")
                connection.execute("INSERT INTO ordinary_app_attempts(operation_id,attempt_original,app_key_reference) VALUES(?,?,?)",
                    (request.operation_id, request.attempt_original(), evidence.raw_proof.device_key_reference))
            connection.commit()

        # Keep the network result in its own transaction before any signer call.
        # Concurrent issuers serialize this exact attempt/key and decode once.
        with closing(self._connect()) as connection:
            connection.execute("BEGIN IMMEDIATE")
            evidence = self._provider.prepare(request, fresh=True)
            row = self._lookup(connection, evidence)
            require(row is not None, "ordinary enrollment reservation missing")
            self._retain_or_check_possession(connection, evidence, reserve=False)
            if row[2] is None:
                decoded = self._provider.decode_integrity(evidence)
                latest = self._provider.prepare(request, fresh=True)
                require(latest.trusted_time_ms >= evidence.trusted_time_ms,
                        "ordinary trusted time regressed before Google retention")
                original = b"" if decoded is None else decoded.google_response
                self._provider.retained_integrity(latest, original,
                    verified_at_ms=latest.trusted_time_ms, fresh=True)
                connection.execute("UPDATE ordinary_app_attempts SET google_original=?,google_original_sha256=?,google_verified_at_ms=? WHERE operation_id=?",
                    (original, hashlib.sha256(original).digest(), latest.trusted_time_ms, request.operation_id))
            self._recheck_database()
            connection.commit()

        # Freeze exact issuance time/input durably, independently of publication.
        with closing(self._connect()) as connection:
            connection.execute("BEGIN IMMEDIATE")
            evidence = self._provider.prepare(request, fresh=True)
            row = self._lookup(connection, evidence)
            require(row is not None and row[2] is not None, "ordinary Google retention missing")
            self._retain_or_check_possession(connection, evidence, reserve=False)
            if row[5] is None:
                integrity = self._provider.retained_integrity(evidence, row[2],
                    verified_at_ms=row[4], fresh=True)
                issued = evidence.trusted_time_ms
                expires = min(evidence.policy.profile_expires_at_ms,
                              issued + evidence.policy.maximum_credential_lifetime_ms)
                signing_input = evidence.signing_request(issued, expires, integrity)
                connection.execute("UPDATE ordinary_app_attempts SET signing_request=? WHERE operation_id=?",
                                   (signing_input, request.operation_id))
            else:
                self._checked_saved_input(connection, evidence, row, fresh=True)
            connection.commit()

        with closing(self._connect()) as connection:
            connection.execute("BEGIN IMMEDIATE")
            evidence = self._provider.prepare(request, fresh=True)
            row = self._lookup(connection, evidence)
            require(row is not None, "ordinary signing reservation missing")
            signing_input = self._checked_saved_input(connection, evidence, row, fresh=True)
            if row[6] is None:
                certificate = self._encoder._encode(evidence, signing_input)
                # Recheck the same actual Native policy and all raw equations
                # after child signing, before publication. No signer output is
                # returned if authority, evidence, clock or custody changed.
                latest = self._provider.prepare(request, fresh=True)
                require(latest.trusted_time_ms >= evidence.trusted_time_ms,
                        "ordinary trusted time regressed before publication")
                self._checked_saved_input(connection, latest, row, fresh=True)
                require(type(certificate) is bytes and 0 < len(certificate) <= MAX_CERTIFICATE_BYTES,
                        "ordinary canonical encoder returned invalid original")
                connection.execute("UPDATE ordinary_app_attempts SET certificate=?,certificate_sha256=? WHERE operation_id=?",
                    (certificate, hashlib.sha256(certificate).digest(), request.operation_id))
            else:
                certificate = row[6]
            self._recheck_database()
            connection.commit()
            return certificate
