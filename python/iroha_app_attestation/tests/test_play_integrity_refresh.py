"""Actual Core/Android equations over synthetic retained issuer subjects.

Google payloads and the canonical credential archive are fixtures here. These
tests do not provide Native enrollment, a live verdict or physical admission.
"""
import hashlib
import json
import unittest
from dataclasses import replace

from iroha_app_attestation.attestation import AttestationRejected
from iroha_app_attestation.play_integrity import _verify_google_payload, request_hash_text
from iroha_app_attestation.play_integrity_refresh import (
    CHALLENGE_DOMAIN, CREDENTIAL_DOMAIN, LEASE_BODY_BYTES, POSSESSION_DOMAIN,
    PlayIntegrityRefreshChallenge, authenticate_refresh_transport,
    decode_refresh_transport, refresh_lease_signing_request, verify_refresh_possession,
)
import test_ordinary_issuance as issuance_fixture


class PlayIntegrityRefreshTests(unittest.TestCase):
    def setUp(self):
        # Reuse only fixture setup; its tests are not inherited or counted here.
        self.f = issuance_fixture.OrdinaryIssuanceTests(); self.f.setUp(); self.addCleanup(self.f.tearDown)
        f = self.f; evidence = f.provider.prepare(f.request, fresh=True)
        initial = _verify_google_payload(f.google_body, f.integrity,
            f.subject.play_integrity_request_hash(hashlib.sha256(f.point).digest()), f.now, b'\x61'*32)
        self.frame = evidence.signing_request(f.now, f.now+3600000, initial)
        self.certificate = b'synthetic canonical archive selected only by this fixture'
        digest = hashlib.sha256(CREDENTIAL_DOMAIN + len(self.certificate).to_bytes(8,'little') + self.certificate).digest()
        self.challenge = PlayIntegrityRefreshChallenge(digest, hashlib.sha256(f.point).digest(),
            f.subject.account_binding, f.subject.network_id, f.subject.lane_id, f.subject.release_id,
            f.subject.hardware_profile_id, f.subject.suite_id, f.subject.trust_policy_digest,
            f.subject.app_authority_policy_digest, f.integrity.policy_digest, b'\x71'*32,
            f.subject.attestation_challenge(), f.subject.policy_epoch, f.subject.hardware_epoch,
            f.now+1, f.now+120001)
        f.now += 2

    def signed(self, selected=None):
        selected = selected or self.challenge; f = self.f
        (f.directory/'refresh-message').write_bytes(selected.signing_bytes())
        f.run_openssl('pkeyutl','-sign','-keyform','DER','-inkey','issuer.der','-rawin',
                      '-in','refresh-message','-out','refresh-core-signature')
        return selected.signing_bytes()[-450:] + (f.directory/'refresh-core-signature').read_bytes()

    def possession(self, message=None):
        f = self.f
        (f.directory/'refresh-possession').write_bytes(message or self.challenge.possession_message())
        f.run_openssl('dgst','-sha256','-sign','leaf.key','-out','refresh-possession.der','refresh-possession')
        return (f.directory/'refresh-possession.der').read_bytes()

    def proof(self):
        f = self.f; value = json.loads(f.google_body)
        value['tokenPayloadExternal']['requestDetails']['requestHash'] = request_hash_text(self.challenge.request_hash())
        value['tokenPayloadExternal']['requestDetails']['timestampMillis'] = str(f.now)
        return _verify_google_payload(json.dumps(value).encode(), f.integrity,
            self.challenge.request_hash(), f.now, b'\x62'*32)

    def select(self, selected=None, *, frame=None, certificate=None, now=None, fresh=True, policy=None):
        return (selected or self.challenge).select_issued_original(signing_request=frame or self.frame,
            certificate=certificate or self.certificate, policy=policy or self.f.policy,
            now_ms=self.f.now if now is None else now, fresh=fresh)

    def test_fixed514_core_transport_actual_signature_and_full_attempt(self):
        original = self.signed(); selected = self.challenge
        self.assertEqual(len(original),514); self.assertEqual(decode_refresh_transport(original),selected)
        self.assertEqual(authenticate_refresh_transport(original, public_key=self.f.public,
            openssl_path=self.f.openssl), selected)
        self.assertEqual(selected.signing_bytes(), CHALLENGE_DOMAIN + (450).to_bytes(8,'little') + original[:450])
        self.assertEqual(selected.attempt_id(), hashlib.sha256(selected.signing_bytes()).digest())
        for change in ({'hardware_epoch':23}, {'nonce':b'\x72'*32}, {'credential_digest':b'\x73'*32}):
            altered = replace(selected,**change)
            self.assertNotEqual(altered.attempt_id(),selected.attempt_id())
            self.assertNotEqual(altered.request_hash(),selected.request_hash())
        for malformed in (original[:-1], original+b'\0', bytes(515)):
            with self.assertRaises(AttestationRejected):decode_refresh_transport(malformed)
        with self.assertRaisesRegex(AttestationRejected,'signature rejected'):
            authenticate_refresh_transport(original[:-1]+bytes([original[-1]^1]),
                public_key=self.f.public,openssl_path=self.f.openssl)

    def test_exact_retained_credential_scope_rejects_policy_key_epoch_and_archive_substitution(self):
        self.assertEqual(self.select(),self.f.point)
        for change in ({'credential_digest':b'\x81'*32}, {'attested_key_id':b'\x82'*32},
                       {'account_binding':b'\x83'*32}, {'lane_id':b'\x84'*32},
                       {'suite_id':b'\x85'*32}, {'hardware_epoch':23}, {'policy_epoch':24},
                       {'original_enrollment_challenge_digest':b'\x86'*32}):
            with self.subTest(change=change),self.assertRaises(AttestationRejected):
                self.select(replace(self.challenge,**change))
        with self.assertRaises(AttestationRejected):self.select(certificate=self.certificate+b'\0')
        with self.assertRaises(AttestationRejected):self.select(policy=replace(self.f.policy,play_integrity_policy=None))
        with self.assertRaises(AttestationRejected):self.select(policy=replace(self.f.policy,circuit_issuer_public_key=b'\x04'+b'\x99'*64))
        altered=bytearray(self.frame); altered[5+746]^=1
        with self.assertRaises(AttestationRejected):self.select(frame=bytes(altered))

    def test_real_android_refresh_signature_rejects_enrollment_message_double_hash_and_wrong_nonce(self):
        original = self.possession()
        verify_refresh_possession(self.challenge,self.f.point,original,self.f.openssl)
        message = self.challenge.possession_message()
        self.assertEqual(message,POSSESSION_DOMAIN+len(self.challenge.signing_bytes()).to_bytes(8,'little')
                         +self.challenge.signing_bytes())
        wrong_messages = (self.f.subject.signing_bytes(),hashlib.sha256(message).digest(),
                          replace(self.challenge,nonce=b'\x91'*32).possession_message())
        for wrong in wrong_messages:
            with self.assertRaisesRegex(AttestationRejected,'signature rejected'):
                verify_refresh_possession(self.challenge,self.f.point,self.possession(wrong),self.f.openssl)

    def test_krpi402_body_preserves_google_original_possession_and_immutable_epochs(self):
        original = self.possession(); proof = self.proof(); f = self.f
        refresh_before=f.now+f.integrity.maximum_refresh_interval_ms
        frame = refresh_lease_signing_request(self.challenge,original,proof,f.policy,
            verified_at_ms=f.now,issued_at_ms=f.now,expires_at_ms=refresh_before,credential_expires_at_ms=f.now+3600000)
        self.assertEqual(frame[:5],b'KRPI\x01'); body=frame[5:407]
        self.assertEqual(len(body),LEASE_BODY_BYTES); self.assertEqual(body[:2],b'\x01\x00')
        self.assertEqual(body[34:66],self.challenge.attempt_id())
        self.assertEqual(body[226:258],proof.request_hash)
        self.assertEqual(body[258:290],proof.google_response_sha256)
        self.assertEqual(body[322:354],hashlib.sha256(original).digest())
        self.assertEqual(int.from_bytes(body[354:362],'little'),self.challenge.policy_epoch)
        self.assertEqual(int.from_bytes(body[362:370],'little'),self.challenge.hardware_epoch)
        self.assertEqual(int.from_bytes(frame[407:409],'little'),len(original))
        self.assertEqual(frame[409:-97],original); self.assertEqual(frame[-97:-65],f.public)
        self.assertEqual(frame[-65:],f.policy.circuit_issuer_public_key)
        for altered in (replace(proof,request_hash=b'\xa1'*32),replace(proof,policy_digest=b'\xa2'*32)):
            with self.assertRaises(AttestationRejected):refresh_lease_signing_request(self.challenge,original,
                altered,f.policy,verified_at_ms=f.now,issued_at_ms=f.now,expires_at_ms=refresh_before,
                credential_expires_at_ms=f.now+3600000)

    def test_cold_original_selection_does_not_renew_expired_challenge_or_credential(self):
        expiry=self.challenge.expires_at_ms
        with self.assertRaises(AttestationRejected):self.select(now=expiry)
        self.assertEqual(self.select(now=expiry,fresh=False),self.f.point)
        with self.assertRaises(AttestationRejected):self.select(now=self.f.now+3600000,fresh=False)
        with self.assertRaises(AttestationRejected):self.select(now=self.challenge.issued_at_ms-1,fresh=False)
