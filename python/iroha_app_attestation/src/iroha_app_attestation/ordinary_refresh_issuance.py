"""Durable periodic Google refresh from the issuer's retained enrollment originals."""
from __future__ import annotations

import hashlib
from contextlib import closing
from dataclasses import dataclass

from .attestation import require
from .ordinary_issuance import DurableOrdinaryCredentialIssuer
from .play_integrity import _verify_google_payload
from .play_integrity_refresh import (authenticate_refresh_transport, decode_refresh_transport,
                                     refresh_lease_signing_request, verify_refresh_possession)


@dataclass(frozen=True)
class OrdinaryIntegrityRefreshRequest:
    operation: str
    operation_id: bytes
    signed_refresh_challenge: bytes
    signature_der: bytes
    play_integrity_token: str

    def validate(self):
        require(self.operation in ('issue','recover') and type(self.operation_id) is bytes
                and len(self.operation_id)==32,"refresh operation absent")
        selected=decode_refresh_transport(self.signed_refresh_challenge)
        require(self.operation_id==selected.attempt_id() and type(self.signature_der) is bytes
                and 8 <= len(self.signature_der) <= 72
                and type(self.play_integrity_token) is str and 0 < len(self.play_integrity_token) <= 64*1024
                and all(33 <= ord(value) <= 126 for value in self.play_integrity_token),
                "refresh originals differ from signed attempt")
        return selected

    def attempt_original(self) -> bytes:
        self.validate()
        return b'iroha:kagemusha:v1:issuer-integrity-refresh-attempt\0'+b''.join(
            len(value).to_bytes(8,'little')+value for value in
            (self.signed_refresh_challenge,self.signature_der,self.play_integrity_token.encode('ascii')))


class DurableOrdinaryIntegrityRefreshIssuer:
    def __init__(self,issuer:DurableOrdinaryCredentialIssuer):
        require(type(issuer) is DurableOrdinaryCredentialIssuer,"retained ordinary issuer owner required")
        self._issuer=issuer
        with closing(issuer._connect()) as connection:
            connection.execute('''CREATE TABLE IF NOT EXISTS ordinary_integrity_refresh_attempts (
                operation_id BLOB PRIMARY KEY NOT NULL, attempt_original BLOB NOT NULL,
                credential_digest BLOB NOT NULL, google_original BLOB,
                google_original_sha256 BLOB, google_verified_at_ms INTEGER,
                signing_request BLOB, lease BLOB, lease_sha256 BLOB)''')

    def _select(self,request:OrdinaryIntegrityRefreshRequest,*,fresh:bool):
        require(type(request) is OrdinaryIntegrityRefreshRequest,"refresh request absent")
        selected=request.validate()
        evidence,signing,certificate,point=self._issuer._retained_refresh_enrollment(selected,fresh=fresh)
        authenticated=authenticate_refresh_transport(request.signed_refresh_challenge,
            public_key=evidence.policy.core_preparation_public_key,openssl_path=self._issuer._provider._openssl)
        require(authenticated==selected,"refresh Core original differs")
        verify_refresh_possession(selected,point,request.signature_der,self._issuer._provider._openssl)
        require(self._issuer._provider._google is not None,"governed Google decoder absent")
        return selected,evidence,signing

    @staticmethod
    def _lookup(connection,request,selected):
        row=connection.execute('SELECT attempt_original,credential_digest,google_original,'
            'google_original_sha256,google_verified_at_ms,signing_request,lease,lease_sha256 '
            'FROM ordinary_integrity_refresh_attempts WHERE operation_id=?',(request.operation_id,)).fetchone()
        if row is not None:
            require(tuple(row[:2])==(request.attempt_original(),selected.credential_digest),
                    "conflicting refresh retry")
            require((row[2] is None)==(row[3] is None)==(row[4] is None),"corrupt retained refresh Google result")
            if row[2] is not None:
                require(type(row[2]) is bytes and 0 < len(row[2]) <= 128*1024
                        and hashlib.sha256(row[2]).digest()==row[3],"corrupt refresh Google original")
            require((row[6] is None)==(row[7] is None),"corrupt retained refresh lease")
            if row[6] is not None:
                require(row[5] is not None and type(row[6]) is bytes and 0 < len(row[6]) <= 4096
                        and hashlib.sha256(row[6]).digest()==row[7],"corrupt refresh lease original")
        return row

    def _proof(self,request,selected,evidence,row):
        require(row is not None and row[2] is not None and type(row[4]) is int
                and selected.issued_at_ms <= row[4] < selected.expires_at_ms
                and row[4] <= evidence.trusted_time_ms,"retained refresh verification time differs")
        return _verify_google_payload(row[2],evidence.policy.play_integrity_policy,selected.request_hash(),
            row[4],hashlib.sha256(request.play_integrity_token.encode('ascii')).digest())

    def _saved_input(self,request,selected,evidence,signing,row):
        original=row[5]
        require(type(original) is bytes and 449 <= len(original) <= 513
                and original[:5]==b'KRPI\x01',"retained refresh signing input absent")
        body=original[5:407]
        issued=int.from_bytes(body[386:394],'little');expires=int.from_bytes(body[394:402],'little')
        require(issued <= evidence.trusted_time_ms < expires,"retained refresh lease interval expired")
        credential_expires=int.from_bytes(signing[5+669:5+677],'little')
        expected=refresh_lease_signing_request(selected,request.signature_der,
            self._proof(request,selected,evidence,row),evidence.policy,verified_at_ms=row[4],
            issued_at_ms=issued,expires_at_ms=expires,credential_expires_at_ms=credential_expires)
        require(original==expected,"retained refresh signing original differs")
        return original

    def refresh(self,request:OrdinaryIntegrityRefreshRequest) -> bytes:
        selected,evidence,signing=self._select(request,fresh=False)
        with closing(self._issuer._connect()) as connection:
            connection.execute('BEGIN IMMEDIATE')
            row=self._lookup(connection,request,selected)
            if row is not None and row[6] is not None:
                self._saved_input(request,selected,evidence,signing,row)
                self._issuer._recheck_database();connection.commit();return row[6]
            require(request.operation=='issue',"refresh lease is unavailable for recovery")
            selected,evidence,signing=self._select(request,fresh=True)
            if row is None:
                connection.execute('INSERT INTO ordinary_integrity_refresh_attempts('
                    'operation_id,attempt_original,credential_digest) VALUES(?,?,?)',
                    (request.operation_id,request.attempt_original(),selected.credential_digest))
            connection.commit()

        # An exact successful Google response is committed before any signing.
        with closing(self._issuer._connect()) as connection:
            connection.execute('BEGIN IMMEDIATE')
            selected,evidence,signing=self._select(request,fresh=True)
            row=self._lookup(connection,request,selected)
            require(row is not None,"refresh reservation absent")
            if row[2] is None:
                self._issuer._provider._recheck()
                decoded=self._issuer._provider._google.decode(request.play_integrity_token,
                    evidence.policy.play_integrity_policy,selected.request_hash(),evidence.trusted_time_ms)
                selected,latest,signing=self._select(request,fresh=True)
                require(latest.trusted_time_ms >= evidence.trusted_time_ms,"refresh trusted clock regressed")
                _verify_google_payload(decoded.google_response,latest.policy.play_integrity_policy,
                    selected.request_hash(),latest.trusted_time_ms,
                    hashlib.sha256(request.play_integrity_token.encode('ascii')).digest())
                connection.execute('UPDATE ordinary_integrity_refresh_attempts SET google_original=?,'
                    'google_original_sha256=?,google_verified_at_ms=? WHERE operation_id=?',
                    (decoded.google_response,hashlib.sha256(decoded.google_response).digest(),
                     latest.trusted_time_ms,request.operation_id))
            self._issuer._recheck_database();connection.commit()

        with closing(self._issuer._connect()) as connection:
            connection.execute('BEGIN IMMEDIATE')
            selected,evidence,signing=self._select(request,fresh=True)
            row=self._lookup(connection,request,selected)
            require(row is not None,"refresh reservation absent")
            if row[5] is None:
                issued=evidence.trusted_time_ms
                credential_expires=int.from_bytes(signing[5+669:5+677],'little')
                expires=min(row[4]+evidence.policy.play_integrity_policy.maximum_refresh_interval_ms,
                    credential_expires,evidence.policy.profile_expires_at_ms)
                original=refresh_lease_signing_request(selected,request.signature_der,
                    self._proof(request,selected,evidence,row),evidence.policy,verified_at_ms=row[4],
                    issued_at_ms=issued,expires_at_ms=expires,credential_expires_at_ms=credential_expires)
                connection.execute('UPDATE ordinary_integrity_refresh_attempts SET signing_request=? '
                    'WHERE operation_id=?',(original,request.operation_id))
            else:self._saved_input(request,selected,evidence,signing,row)
            self._issuer._recheck_database();connection.commit()

        with closing(self._issuer._connect()) as connection:
            connection.execute('BEGIN IMMEDIATE')
            selected,evidence,signing=self._select(request,fresh=True)
            row=self._lookup(connection,request,selected)
            require(row is not None,"refresh reservation absent")
            original=self._saved_input(request,selected,evidence,signing,row)
            if row[6] is None:
                lease=self._issuer._encoder._encode_refresh(evidence,original)
                selected,latest,signing=self._select(request,fresh=True)
                require(latest.trusted_time_ms >= evidence.trusted_time_ms,"refresh trusted clock regressed")
                self._saved_input(request,selected,latest,signing,row)
                require(type(lease) is bytes and 0 < len(lease) <= 4096,"canonical refresh encoder failed")
                connection.execute('UPDATE ordinary_integrity_refresh_attempts SET lease=?,lease_sha256=? '
                    'WHERE operation_id=?',(lease,hashlib.sha256(lease).digest(),request.operation_id))
            else:lease=row[6]
            self._issuer._recheck_database();connection.commit();return lease
