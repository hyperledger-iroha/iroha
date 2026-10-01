"""Fixed first-release ordinary enrollment transport shared with Iroha's model.

Core retains the actual AccountId and produces its model-owned account binding.
This module authenticates the fixed signed transport, without a second AccountId
or Norito codec. Release and policy admission belong to the genuine Native owner.
"""
from __future__ import annotations
import hashlib
import subprocess
import tempfile
from dataclasses import dataclass
from pathlib import Path
from .attestation import (RawPlatformProof, children, der_one, device_key_reference,
                          fixed32, positive_integer, public_key_pem, require, verify_apple_assertion)
from .play_integrity import PlayIntegrityPolicy, PlayIntegrityProof

CHALLENGE_DOMAIN = b'iroha:kagemusha:v1:ordinary-app-enrollment-challenge\0'
INTEGRITY_DOMAIN = b'iroha:kagemusha:v1:play-integrity-enrollment\0'
POSSESSION_DOMAIN = b'iroha:kagemusha:v1:app-enrollment-possession\0'
POSSESSION_BODY_BYTES = 371
EVIDENCE_DOMAIN = b'iroha:kagemusha:v1:ordinary-app-enrollment-evidence\0'
CHALLENGE_BODY_BYTES = 451
CHALLENGE_TRANSPORT_BYTES = 515
CREDENTIAL_BODY_BYTES = 794
SIGNING_REQUEST_MAGIC = b'KOAC\x01'
SIGNING_REQUEST_BYTES = 831
_FIELDS = ('enrollment_id', 'client_nonce', 'server_nonce', 'account_binding',
           'network_id', 'lane_id', 'release_id', 'hardware_profile_id', 'suite_id',
           'trust_policy_digest', 'app_authority_policy_digest',
           'financial_authority_commitment', 'issuer_policy_digest')


@dataclass(frozen=True)
class OrdinaryEnrollmentChallenge:
    """Decoded signing subject; parsing alone does not authenticate it."""
    platform_class: int
    enrollment_id: bytes
    client_nonce: bytes
    server_nonce: bytes
    account_binding: bytes
    network_id: bytes
    lane_id: bytes
    release_id: bytes
    hardware_profile_id: bytes
    suite_id: bytes
    trust_policy_digest: bytes
    app_authority_policy_digest: bytes
    financial_authority_commitment: bytes
    issuer_policy_digest: bytes
    policy_epoch: int
    hardware_epoch: int
    issued_at_ms: int
    expires_at_ms: int

    def signing_bytes(self) -> bytes:
        require(type(self.platform_class) is int and self.platform_class in (1, 2),
                'invalid ordinary platform class')
        fields = b''.join(fixed32(getattr(self, name), name) for name in _FIELDS)
        times = (self.policy_epoch, self.hardware_epoch, self.issued_at_ms, self.expires_at_ms)
        require(all(type(value) is int and 0 < value < (1 << 64) for value in times)
                and self.client_nonce != self.server_nonce
                and 0 < self.expires_at_ms - self.issued_at_ms <= 120_000,
                'invalid ordinary enrollment scope or time')
        body = b'\x01\x00' + bytes([self.platform_class]) + fields + b''.join(value.to_bytes(8, 'little') for value in times)
        require(len(body) == CHALLENGE_BODY_BYTES, 'ordinary challenge layout changed')
        return CHALLENGE_DOMAIN + CHALLENGE_BODY_BYTES.to_bytes(8, 'little') + body

    def attestation_challenge(self) -> bytes:
        return hashlib.sha256(self.signing_bytes()).digest()

    def play_integrity_request_hash(self, attested_key_id: bytes) -> bytes:
        return hashlib.sha256(INTEGRITY_DOMAIN + self.signing_bytes()
                              + fixed32(attested_key_id, 'actual attested key ID')).digest()

    def possession_message(self, attested_key_id: bytes, raw_attestation_sha256: bytes,
                           issued_at_ms: int, expires_at_ms: int) -> bytes:
        """Exact native E371 projection, never a pending admission or credential.

        The caller supplies the exact original signed C interval, never a renewed
        lease. This formatter cannot authenticate C, pending raw-attestation admission
        or issuer custody; their originals must be verified independently by its caller.
        """
        self.signing_bytes()
        require(type(issued_at_ms) is int and type(expires_at_ms) is int
                and 0 < issued_at_ms < expires_at_ms < (1 << 64)
                and expires_at_ms - issued_at_ms <= 120_000
                and issued_at_ms == self.issued_at_ms and expires_at_ms == self.expires_at_ms,
                'enrollment possession interval differs from original C')
        # The sole E first field binds the complete original signed C, including
        # epochs, suite, trust and financial commitment. Stable C.enrollment_id
        # remains the separate native21 selector; there is no EID-wire fallback.
        fields = (self.attestation_challenge(), self.client_nonce, self.server_nonce,
                  self.account_binding, self.network_id, self.app_authority_policy_digest,
                  self.release_id, self.hardware_profile_id, self.lane_id,
                  attested_key_id, raw_attestation_sha256)
        body = b'\x01\x00\x01' + b''.join(fixed32(value, 'native E selector') for value in fields)
        body += issued_at_ms.to_bytes(8, 'little') + expires_at_ms.to_bytes(8, 'little')
        require(len(body) == POSSESSION_BODY_BYTES, 'enrollment possession E layout changed')
        return POSSESSION_DOMAIN + POSSESSION_BODY_BYTES.to_bytes(8, 'little') + body


def decode_challenge_transport(original: bytes) -> OrdinaryEnrollmentChallenge:
    """Decode only the sole fixed515 body/signature layout; no former507 fallback."""
    require(type(original) is bytes and len(original) == CHALLENGE_TRANSPORT_BYTES
            and original[:2] == b'\x01\x00', 'invalid ordinary signed challenge transport')
    body = original[:CHALLENGE_BODY_BYTES]
    selectors = [body[3+index*32:3+(index+1)*32] for index in range(13)]
    times = [int.from_bytes(body[419+index*8:427+index*8], 'little') for index in range(4)]
    subject = OrdinaryEnrollmentChallenge(body[2], *selectors, *times)
    require(subject.signing_bytes()[-CHALLENGE_BODY_BYTES:] == body, 'ordinary challenge body differs')
    return subject


def authenticate_challenge_transport(original: bytes, expected: OrdinaryEnrollmentChallenge,
                                     core_public_key: bytes, trusted_time_ms: int,
                                     openssl_path: Path, *, fresh: bool = True) -> OrdinaryEnrollmentChallenge:
    """Check real Core Ed signature and exact independently selected Native scope."""
    subject = decode_challenge_transport(original)
    require(type(expected) is OrdinaryEnrollmentChallenge and subject == expected,
            'ordinary preparation differs from selected Native subject')
    require(type(trusted_time_ms) is int and 0 < trusted_time_ms < (1 << 64)
            and type(fresh) is bool and subject.issued_at_ms <= trusted_time_ms
            and (not fresh or trusted_time_ms < subject.expires_at_ms),
            'ordinary preparation expired or from the future')
    key = fixed32(core_public_key, 'independently selected Core preparation key')
    require(isinstance(openssl_path, Path) and openssl_path.is_absolute() and openssl_path.is_file(),
            'invalid ordinary signature verification environment')
    with tempfile.TemporaryDirectory(prefix='iroha-ordinary-preparation-') as temporary:
        directory = Path(temporary)
        (directory/'public.pem').write_bytes(public_key_pem(bytes.fromhex('302a300506032b6570032100') + key))
        (directory/'message.bin').write_bytes(subject.signing_bytes())
        (directory/'signature.bin').write_bytes(original[CHALLENGE_BODY_BYTES:])
        result = subprocess.run([str(openssl_path), 'pkeyutl', '-verify', '-pubin', '-inkey', str(directory/'public.pem'),
                                 '-rawin', '-in', str(directory/'message.bin'), '-sigfile', str(directory/'signature.bin')],
                                stdin=subprocess.DEVNULL, capture_output=True, timeout=5, check=False)
        require(result.returncode == 0, 'ordinary preparation signature rejected')
    return subject


@dataclass(frozen=True)
class OrdinaryPlatformEvidenceChallenge:
    """Raw verifier adapter, used only after actual preparation authentication."""
    subject: OrdinaryEnrollmentChallenge
    attested_key_id: bytes
    def transcript(self) -> bytes:
        # Both platform verifiers hash this exact model signing message.
        require(type(self.attested_key_id) is bytes and len(self.attested_key_id) == 32,
                'invalid platform evidence key ID')
        return self.subject.signing_bytes()


@dataclass(frozen=True)
class EnrollmentPossession:
    """Checked platform equation; it confers no signer or monetary authority."""
    attested_key_id: bytes
    raw_attestation_sha256: bytes
    raw_possession_sha256: bytes
    platform_evidence_digest: bytes
    app_attest_counter_floor: int


def verify_enrollment_possession(challenge: OrdinaryEnrollmentChallenge, proof: RawPlatformProof,
                                 raw_attestation: bytes, raw_possession: bytes,
                                 openssl_path: Path, *, apple_app_id: str | None,
                                 possession_issued_at_ms: int, possession_expires_at_ms: int) -> EnrollmentPossession:
    """Verify possession of the exact attested key and commit both originals."""
    require(type(proof) is RawPlatformProof and type(raw_attestation) is bytes
            and 0 < len(raw_attestation) <= 128 * 1024
            and hashlib.sha256(raw_attestation).digest() == proof.evidence_sha256
            and type(raw_possession) is bytes and 0 < len(raw_possession) <= 64 * 1024,
            'ordinary possession original differs from attested evidence')
    point = proof.attested_public_key_sec1
    require(proof.device_key_reference == device_key_reference(point), 'ordinary possession key differs')
    key_id = hashlib.sha256(point).digest()
    message = challenge.possession_message(key_id, proof.evidence_sha256,
                                           possession_issued_at_ms, possession_expires_at_ms)
    if challenge.platform_class == 1:
        require(proof.platform == 'android_keymint' and apple_app_id is None
                and 8 <= len(raw_possession) <= 72, 'invalid Android enrollment possession')
        scalars = children(der_one(raw_possession))
        require(len(scalars) == 2 and all(0 < positive_integer(value) < (1 << 256) for value in scalars),
                'invalid Android possession signature')
        require(isinstance(openssl_path, Path) and openssl_path.is_absolute() and openssl_path.is_file(),
                'invalid possession verification environment')
        with tempfile.TemporaryDirectory(prefix='iroha-ordinary-possession-') as temporary:
            directory = Path(temporary)
            spki = bytes.fromhex('3059301306072a8648ce3d020106082a8648ce3d030107034200') + point
            (directory/'public.pem').write_bytes(public_key_pem(spki))
            (directory/'message.bin').write_bytes(message)
            (directory/'signature.bin').write_bytes(raw_possession)
            result = subprocess.run([str(openssl_path), 'dgst', '-sha256', '-verify', str(directory/'public.pem'),
                                     '-signature', str(directory/'signature.bin'), str(directory/'message.bin')],
                                    stdin=subprocess.DEVNULL, capture_output=True, timeout=5, check=False)
            require(result.returncode == 0, 'Android enrollment possession signature rejected')
        floor = 0
    else:
        require(proof.platform == 'apple_app_attest' and type(apple_app_id) is str,
                'invalid Apple enrollment possession')
        assertion = verify_apple_assertion(raw_possession, message, message, point, key_id,
                                           apple_app_id, 0, openssl_path,
                                           expected_validation_category=None, expected_bundle_version=None)
        floor = assertion.counter
    evidence = EVIDENCE_DOMAIN + len(raw_attestation).to_bytes(8, 'little') + raw_attestation
    evidence += len(raw_possession).to_bytes(8, 'little') + raw_possession
    return EnrollmentPossession(key_id, proof.evidence_sha256, hashlib.sha256(raw_possession).digest(),
                                hashlib.sha256(evidence).digest(), floor)


def credential_signing_request(challenge: OrdinaryEnrollmentChallenge, proof: RawPlatformProof,
                               possession: EnrollmentPossession,
                               app_identity_digest: bytes, app_release_digest: bytes,
                               authority_public_key: bytes, issued_at_ms: int, expires_at_ms: int,
                               maximum_lifetime_ms: int, profile_expires_at_ms: int,
                               allowed_android_security_levels: frozenset[int],
                               integrity_policy: PlayIntegrityPolicy | None,
                               integrity_proof: PlayIntegrityProof | None) -> bytes:
    """Frame the model-owned encoder input after actual Native/platform/PI checks.

    Calling this formatter does not verify evidence or grant signer custody.
    The provider must supply proof from its real verifier and the independently
    admitted policy, then recheck that same scope immediately before signing.
    """
    challenge.signing_bytes()
    require(type(proof) is RawPlatformProof, 'raw platform proof is absent')
    point = proof.attested_public_key_sec1
    require(type(point) is bytes and len(point) == 65 and point[0] == 4
            and proof.device_key_reference == device_key_reference(point), 'ordinary attested key differs')
    key_id = hashlib.sha256(point).digest()
    require(type(possession) is EnrollmentPossession and possession.attested_key_id == key_id
            and possession.raw_attestation_sha256 == proof.evidence_sha256,
            'ordinary possession differs from actual raw proof')
    app_attest_counter_floor = possession.app_attest_counter_floor
    require(type(app_attest_counter_floor) is int and 0 <= app_attest_counter_floor < (1 << 32),
            'invalid independent App Attest counter floor')
    if challenge.platform_class == 1:
        require(type(allowed_android_security_levels) is frozenset and bool(allowed_android_security_levels)
                and all(type(level) is int for level in allowed_android_security_levels)
                and allowed_android_security_levels <= {1, 2}
                and proof.platform == 'android_keymint'
                and type(proof.android_security_level) is int
                and proof.android_security_level in allowed_android_security_levels
                and app_attest_counter_floor == 0,
                'ordinary Android security level differs from selected policy')
        level = proof.android_security_level
    else:
        require(proof.platform == 'apple_app_attest' and allowed_android_security_levels == frozenset()
                and integrity_policy is None and integrity_proof is None
                and app_attest_counter_floor > 0,
                'ordinary Apple contains Android policy')
        level = 3
    require(all(type(value) is int and 0 < value < (1 << 64) for value in
                (issued_at_ms, expires_at_ms, maximum_lifetime_ms, profile_expires_at_ms))
            and challenge.issued_at_ms <= issued_at_ms < challenge.expires_at_ms
            and 0 < expires_at_ms - issued_at_ms <= maximum_lifetime_ms
            and expires_at_ms <= profile_expires_at_ms,
            'ordinary credential issue interval differs from selected policy')
    selectors = [challenge.enrollment_id, challenge.client_nonce, challenge.server_nonce,
        challenge.account_binding, challenge.network_id, challenge.lane_id, challenge.release_id,
        challenge.hardware_profile_id, challenge.suite_id, challenge.trust_policy_digest,
        challenge.app_authority_policy_digest, app_identity_digest, app_release_digest, key_id,
        proof.device_key_reference, challenge.financial_authority_commitment, possession.platform_evidence_digest,
        challenge.attestation_challenge()]
    body = b'\x01\x00' + bytes([challenge.platform_class, level]) + b''.join(fixed32(field, 'ordinary credential selector') for field in selectors)
    body += point + b''.join(value.to_bytes(8, 'little') for value in
                            (challenge.policy_epoch, challenge.hardware_epoch, issued_at_ms, expires_at_ms))
    # Apple possession assertion's actual observed counter becomes this floor;
    # the raw attestation's initial counter0 and financial index are separate.
    body += app_attest_counter_floor.to_bytes(4, 'little')
    if integrity_policy is None:
        require(integrity_proof is None, 'unexpected Play Integrity proof')
        body += bytes(113)
    else:
        integrity_policy.validate()
        require(type(integrity_proof) is PlayIntegrityProof
                and integrity_policy.app_signing_certificate_sha256 == app_identity_digest
                and integrity_proof.policy_digest == integrity_policy.policy_digest
                and integrity_proof.request_hash == challenge.play_integrity_request_hash(key_id)
                and challenge.issued_at_ms <= integrity_proof.timestamp_ms <= issued_at_ms
                and issued_at_ms - integrity_proof.timestamp_ms <= integrity_policy.maximum_evidence_age_ms,
                'ordinary Play Integrity binding differs')
        body += b'\x01' + integrity_proof.request_hash + integrity_proof.google_response_sha256 + integrity_proof.policy_digest
        body += issued_at_ms.to_bytes(8, 'little') + min(expires_at_ms, issued_at_ms + integrity_policy.maximum_refresh_interval_ms).to_bytes(8, 'little')
    require(len(body) == CREDENTIAL_BODY_BYTES, 'ordinary credential body layout changed')
    request = SIGNING_REQUEST_MAGIC + body + fixed32(authority_public_key, 'actual governed app authority')
    require(len(request) == SIGNING_REQUEST_BYTES, 'ordinary signing request layout changed')
    return request
