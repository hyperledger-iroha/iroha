"""Real fixture PKIX/Ed/P256 + durable SQLite; scripted encoder/Google transport.

The encoded certificate/lease stand-ins test issuer recovery only. They confer
no Native admission, Google production verdict or monetary qualification.
"""
import base64
import hashlib
import json
import sqlite3
import unittest
from contextlib import closing
from dataclasses import replace
from unittest.mock import patch

import test_ordinary_issuance as enrollment_fixture
from iroha_app_attestation.attestation import AttestationRejected
from iroha_app_attestation.ordinary_refresh_issuance import (DurableOrdinaryIntegrityRefreshIssuer,
                                                           OrdinaryIntegrityRefreshRequest)
from iroha_app_attestation.ordinary_refresh_service import decode_request,SCHEMA,PATH
from iroha_app_attestation.ordinary_service import OrdinaryCredentialService
from iroha_app_attestation.play_integrity import request_hash_text
from iroha_app_attestation.play_integrity_refresh import PlayIntegrityRefreshChallenge,CREDENTIAL_DOMAIN


class OrdinaryRefreshIssuanceTests(unittest.TestCase):
    def setUp(self):
        self.f=enrollment_fixture.OrdinaryIssuanceTests();self.f.setUp();self.addCleanup(self.f.tearDown)
        f=self.f
        with patch('iroha_app_attestation.play_integrity.urllib.request.build_opener') as google, \
                patch('iroha_app_attestation.ordinary_issuance.encode_ordinary_with_iroha',
                    side_effect=lambda frame,*_:b'component-only-certificate:'+hashlib.sha256(frame).digest()):
            google.return_value.open.return_value=enrollment_fixture.Response(f.google_body)
            certificate=f.issuer.issue(f.request)
        self.selected=PlayIntegrityRefreshChallenge(
            hashlib.sha256(CREDENTIAL_DOMAIN+len(certificate).to_bytes(8,'little')+certificate).digest(),
            hashlib.sha256(f.point).digest(),f.subject.account_binding,f.subject.network_id,f.subject.lane_id,
            f.subject.release_id,f.subject.hardware_profile_id,f.subject.suite_id,f.subject.trust_policy_digest,
            f.subject.app_authority_policy_digest,f.integrity.policy_digest,b'\x71'*32,
            f.subject.attestation_challenge(),f.subject.policy_epoch,f.subject.hardware_epoch,f.now,f.now+1000)
        self.request=self.make_request(self.selected)
        self.issuer=DurableOrdinaryIntegrityRefreshIssuer(f.issuer)
        value=json.loads(f.google_body)
        value['tokenPayloadExternal']['requestDetails']['requestHash']=request_hash_text(self.selected.request_hash())
        value['tokenPayloadExternal']['requestDetails']['timestampMillis']=str(f.now)
        self.google_body=json.dumps(value).encode()
        self.inputs=[]

    def make_request(self,selected):
        f=self.f
        (f.directory/'refresh-message').write_bytes(selected.signing_bytes())
        f.run_openssl('pkeyutl','-sign','-keyform','DER','-inkey','issuer.der','-rawin',
            '-in','refresh-message','-out','refresh-core-signature')
        signed=selected.signing_bytes()[-450:]+(f.directory/'refresh-core-signature').read_bytes()
        (f.directory/'refresh-pop').write_bytes(selected.possession_message())
        f.run_openssl('dgst','-sha256','-sign','leaf.key','-out','refresh-pop.der','refresh-pop')
        return OrdinaryIntegrityRefreshRequest('issue',selected.attempt_id(),signed,
            (f.directory/'refresh-pop.der').read_bytes(),'opaque-refresh-token')

    def encode(self,original,*_):
        self.inputs.append(original)
        return b'component-only-lease:'+hashlib.sha256(original).digest()

    def run_refresh(self,request=None,*,encoder=None,body=None):
        with patch('iroha_app_attestation.play_integrity.urllib.request.build_opener') as google, \
                patch('iroha_app_attestation.ordinary_issuance.encode_refresh_with_iroha',
                    side_effect=encoder or self.encode) as signing:
            google.return_value.open.return_value=enrollment_fixture.Response(body or self.google_body)
            result=self.issuer.refresh(request or self.request)
            return result,google.return_value.open.call_count,signing.call_count

    def test_same_attempt_recovery_after_restart_and_challenge_expiry_uses_exact_lease(self):
        original,google,signing=self.run_refresh()
        self.assertEqual((google,signing),(1,1));self.assertEqual(len(self.inputs),1)
        self.f.now+=2000
        self.issuer=DurableOrdinaryIntegrityRefreshIssuer(self.f.issuer)
        recovered,google,signing=self.run_refresh(replace(self.request,operation='recover'))
        self.assertEqual((recovered,google,signing),(original,0,0))
        self.assertEqual(len(self.inputs),1)

    def test_failed_signer_retains_google_and_frozen_signing_input_before_retry(self):
        with self.assertRaisesRegex(RuntimeError,'fixture signer stopped'):
            self.run_refresh(encoder=lambda *_:(_ for _ in ()).throw(RuntimeError('fixture signer stopped')))
        with closing(sqlite3.connect(self.f.path)) as connection:
            row=connection.execute('SELECT google_original,signing_request,lease FROM ordinary_integrity_refresh_attempts').fetchone()
        self.assertEqual(row[0],self.google_body);self.assertTrue(row[1].startswith(b'KRPI\x01'));self.assertIsNone(row[2])
        self.f.now+=1
        original,google,signing=self.run_refresh()
        self.assertEqual((google,signing),(0,1));self.assertEqual(self.inputs,[row[1]])
        self.assertEqual(original,b'component-only-lease:'+hashlib.sha256(row[1]).digest())

    def test_conflicting_token_and_second_valid_der_stop_before_google_or_signing(self):
        self.run_refresh()
        other_signature=self.make_request(self.selected).signature_der
        for changed in (replace(self.request,play_integrity_token='different-token'),
                        replace(self.request,signature_der=other_signature)):
            with patch('iroha_app_attestation.play_integrity.urllib.request.build_opener') as google, \
                    patch('iroha_app_attestation.ordinary_issuance.encode_refresh_with_iroha') as signing:
                with self.assertRaises(AttestationRejected):self.issuer.refresh(changed)
                google.assert_not_called();signing.assert_not_called()

    def test_missing_original_credential_wrong_digest_or_lost_authority_stops_before_decode(self):
        for changed in (replace(self.selected,original_enrollment_challenge_digest=b'\x81'*32),
                        replace(self.selected,credential_digest=b'\x82'*32)):
            offered=self.make_request(changed)
            with patch('iroha_app_attestation.play_integrity.urllib.request.build_opener') as google:
                with self.assertRaises(AttestationRejected):self.issuer.refresh(offered)
                google.assert_not_called()
        self.f.native_current=False
        with self.assertRaises(AttestationRejected):self.run_refresh()
        self.f.native_current=True;self.f.revocation_clear=False
        with self.assertRaises(AttestationRejected):self.run_refresh()

    def test_recover_never_issues_and_expired_lease_never_renews(self):
        with self.assertRaises(AttestationRejected):self.run_refresh(replace(self.request,operation='recover'))
        self.run_refresh();self.f.now+=30001
        with patch('iroha_app_attestation.play_integrity.urllib.request.build_opener') as google, \
                patch('iroha_app_attestation.ordinary_issuance.encode_refresh_with_iroha') as signing:
            with self.assertRaises(AttestationRejected):self.issuer.refresh(replace(self.request,operation='recover'))
            google.assert_not_called();signing.assert_not_called()

    def body(self):
        r=self.request;encode=lambda value:base64.b64encode(value).decode()
        return {'schema':SCHEMA,'operation':r.operation,'operation_id':r.operation_id.hex(),
            'signed_refresh_challenge_base64':encode(r.signed_refresh_challenge),
            'signature_der_base64':encode(r.signature_der),'play_integrity_token':r.play_integrity_token}

    def test_closed_six_field_service_requires_actual_parent_and_preserves_two_original_fields(self):
        value=self.body();body=json.dumps(value).encode()
        self.assertEqual(decode_request(body),self.request)
        context=object();service=OrdinaryCredentialService(issuer=self.f.issuer,refresh_issuer=self.issuer,
            authorize_core_call=lambda offered:offered is context)
        self.assertEqual(service.handle(method='POST',path=PATH,body=body,content_type='application/json',
            transport_context={'claimed_parent':True})[0],401)
        for changed in ({'policy':{}},{'google_verdict':{}},{'credential_base64':'AA=='},
                        {'play_integrity_token':None},{'operation_id':self.selected.credential_digest.hex()}):
            with self.assertRaises(AttestationRejected):decode_request(json.dumps(dict(value,**changed)).encode())
        duplicate=body[:-1]+b',"operation":"issue"}'
        with self.assertRaises(AttestationRejected):decode_request(duplicate)
        with patch('iroha_app_attestation.play_integrity.urllib.request.build_opener') as google, \
                patch('iroha_app_attestation.ordinary_issuance.encode_refresh_with_iroha',side_effect=self.encode):
            google.return_value.open.return_value=enrollment_fixture.Response(self.google_body)
            status,result=service.handle(method='POST',path=PATH,body=body,content_type='application/json',
                transport_context=context)
        self.assertEqual(status,200);result=json.loads(result)
        self.assertEqual(set(result),{'lease_base64','lease_sha256_hex'})
        self.assertEqual(hashlib.sha256(base64.b64decode(result['lease_base64'])).hexdigest(),result['lease_sha256_hex'])
