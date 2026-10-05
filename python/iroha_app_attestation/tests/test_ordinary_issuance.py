from iroha_app_attestation.native_time_interval import NativeTimeInterval
"""Real Core/P256 equations and durable issuer transactions with public mocks.

The tests use a synthetic OEM chain, a simulated deployment policy holder,
mocked Google TLS response and mocked encoder output. They do not establish a
production Native policy, canonical certificate or physical qualification.
"""
import base64
import hashlib
import json
import os
import shutil
import sqlite3
import subprocess
import tempfile
import time
import unittest
import urllib.error
from contextlib import closing
from dataclasses import replace
from pathlib import Path
from unittest.mock import patch

from iroha_app_attestation.attestation import AttestationRejected, encode_android_chain
from iroha_app_attestation.ordinary_enrollment import CHALLENGE_BODY_BYTES, OrdinaryPlatformEvidenceChallenge
from iroha_app_attestation.ordinary_issuance import CanonicalOrdinaryCredentialEncoder, CanonicalRawAppAdmissionEncoder, DurableOrdinaryCredentialIssuer
from iroha_app_attestation.ordinary_service import PATH, SCHEMA, OrdinaryCredentialService
from iroha_app_attestation.ordinary_provider import (GovernedOrdinaryEvidenceProvider,
    OrdinaryCredentialRequest, OrdinaryReleasePolicy)
from iroha_app_attestation.play_integrity import GooglePlayIntegrityVerifier, PlayIntegrityPolicy, request_hash_text
from iroha_app_attestation.provider import OemKeyMintPolicy
from test_ordinary_enrollment import challenge
from test_synthetic_platform_evidence import SignedEnvelope, keymint_description


class Response:
    status = 200
    headers = {'Content-Type': 'application/json'}
    url = 'https://playintegrity.googleapis.com/v1/org.example.wallet:decodeIntegrityToken'
    def __init__(self, body): self.body = body
    def __enter__(self): return self
    def __exit__(self, *args): pass
    def geturl(self): return self.url
    def read(self, bound): return self.body[:bound]


class OrdinaryIssuanceTests(unittest.TestCase):
    def setUp(self):
        self.openssl = Path(shutil.which('openssl')).resolve()
        self.temporary = tempfile.TemporaryDirectory()
        self.directory = Path(self.temporary.name)
        self.now = int(time.time()*1000)
        self.subject = replace(challenge(), issued_at_ms=self.now-200, expires_at_ms=self.now+119800)
        self.native_current = True
        self.revocation_clear = True
        self.rechecks = 0
        self.status_checks = 0
        (self.directory/'issuer.der').write_bytes(bytes.fromhex('302e020100300506032b657004220420')+bytes([73])*32)
        self.run_openssl('pkey','-inform','DER','-in','issuer.der','-pubout','-outform','DER','-out','public.der')
        self.public = (self.directory/'public.der').read_bytes()[-32:]
        fixture = SignedEnvelope(self.directory, self.openssl)
        self.fixture = fixture
        selected = OrdinaryPlatformEvidenceChallenge(self.subject, bytes(32))
        leaf, root = fixture.sign('1.3.6.1.4.1.11129.2.1.17', keymint_description(selected,
            'org.example.wallet',28,b'\x22'*32,security_level=2,keymint_security_level=2))
        self.point = fixture.point
        self.raw = encode_android_chain([leaf,root])
        # Generated fixture certificates may start in the next UTC second.
        # Evaluate them at a real time after generation, never before notBefore.
        self.now = int(time.time()*1000)
        (self.directory/'possession').write_bytes(self.subject.possession_message(hashlib.sha256(self.point).digest(),
            hashlib.sha256(self.raw).digest(), self.subject.issued_at_ms, self.subject.expires_at_ms))
        self.run_openssl('dgst','-sha256','-sign','leaf.key','-out','possession.der','possession')
        self.pop = (self.directory/'possession.der').read_bytes()
        self.platform = OemKeyMintPolicy('org.example.wallet',28,b'\x22'*32,root,
            hashlib.sha256(root).digest(),self.check_revocation,frozenset({1,2}))
        self.integrity = PlayIntegrityPolicy(b'\x55'*32,'org.example.wallet',28,b'\x22'*32,
            5000,30000,True,True,'MEETS_DEVICE_INTEGRITY')
        self.policy = OrdinaryReleasePolicy(self.subject.release_id,self.subject.network_id,
            self.subject.hardware_profile_id,self.subject.suite_id,self.subject.trust_policy_digest,
            self.subject.app_authority_policy_digest,self.subject.issuer_policy_digest,self.subject.policy_epoch,
            min(self.subject.issued_at_ms,self.now-1000),self.now+86400000,b'\x22'*32,b'\x23'*32,self.public,self.public,self.point,
            3600000,self.platform,self.integrity)
        self.request = OrdinaryCredentialRequest('issue',self.subject.attestation_challenge(),
            self.sign_subject(self.subject),self.point,self.raw,self.pop,'android_keystore','opaque-token')
        self.google_body = self.verdict()
        self.provider = self.make_provider()
        self.seed_path = self.directory/'synthetic-seed'
        self.seed_path.write_bytes(bytes([73])*32); self.seed_path.chmod(0o600)
        self.seed_fd = os.open(self.seed_path,os.O_RDONLY)
        self.encoder = CanonicalOrdinaryCredentialEncoder(encoder=self.openssl,
            encoder_sha256=hashlib.sha256(self.openssl.read_bytes()).digest(),
            authority_key_fd=self.seed_fd,authority_public_key=self.public)
        self.raw_encoder = CanonicalRawAppAdmissionEncoder(encoder=self.openssl,
            encoder_sha256=hashlib.sha256(self.openssl.read_bytes()).digest(),
            authority_key_fd=self.seed_fd,authority_public_key=self.public)
        self.path = self.directory/'ordinary.sqlite'
        self.issuer = DurableOrdinaryCredentialIssuer(path=self.path,provider=self.provider,
            encoder=self.encoder,raw_encoder=self.raw_encoder)
        # A mocked native producer returns a real Ed-signed314 transport for the
        # exact pending model body. This is a protocol fixture, not installed
        # issuer/native qualification. Every final test starts after that raw step.
        with patch('iroha_app_attestation.ordinary_issuance.encode_raw_admission_with_iroha',
                   side_effect=self.raw_encoder_original):
            self.raw_original = self.issuer.accept_raw(self.request.raw_request())

    def tearDown(self):
        self.encoder.close(); self.raw_encoder.close(); os.close(self.seed_fd); self.temporary.cleanup()

    def raw_encoder_original(self, request, *args):
        from iroha_app_attestation.ordinary_raw_admission import DOMAIN, BODY_BYTES
        self.assertEqual(len(request),288); self.assertEqual(request[:6],b'KRAC01')
        self.assertEqual(request[-32:],self.public)
        body=request[6:256]
        (self.directory/'raw-message').write_bytes(DOMAIN+BODY_BYTES.to_bytes(8,'little')+body)
        self.run_openssl('pkeyutl','-sign','-keyform','DER','-inkey','issuer.der','-rawin',
                         '-in','raw-message','-out','raw-signature')
        return body+(self.directory/'raw-signature').read_bytes()

    def new_issuer(self):
        return DurableOrdinaryCredentialIssuer(path=self.directory/'new-raw.sqlite',provider=self.provider,
            encoder=self.encoder,raw_encoder=self.raw_encoder)

    def raw_rows(self, path=None):
        with closing(sqlite3.connect(path or self.path)) as connection:
            return connection.execute('SELECT signing_request,admission,admission_sha256 FROM ordinary_raw_attempts').fetchall()

    def run_openssl(self,*args):
        subprocess.run([str(self.openssl),*args],cwd=self.directory,capture_output=True,check=True)

    def sign_subject(self, subject):
        (self.directory/'message').write_bytes(subject.signing_bytes())
        self.run_openssl('pkeyutl','-sign','-keyform','DER','-inkey','issuer.der','-rawin','-in','message','-out','signature')
        return subject.signing_bytes()[-CHALLENGE_BODY_BYTES:] + (self.directory/'signature').read_bytes()

    def recheck(self):
        self.rechecks += 1
        if not self.native_current: raise AttestationRejected('synthetic Native owner no longer current')

    def check_revocation(self, chain, now):
        self.status_checks += 1
        self.assertEqual(tuple(chain),tuple(__import__('iroha_app_attestation.attestation',fromlist=['decode_android_chain']).decode_android_chain(self.raw)))
        self.assertEqual(now,self.now)
        return self.revocation_clear

    def make_provider(self, policy=None):
        return GovernedOrdinaryEvidenceProvider(policies=(policy or self.policy,),
            trusted_time_interval=lambda:NativeTimeInterval(self.now,self.now),recheck_native_policy=self.recheck,openssl_path=self.openssl,
            play_integrity=GooglePlayIntegrityVerifier(lambda:'synthetic-service-access-token'))

    def verdict(self):
        request_hash = self.subject.play_integrity_request_hash(hashlib.sha256(self.point).digest())
        return json.dumps({'tokenPayloadExternal':{
            'requestDetails':{'requestPackageName':'org.example.wallet','requestHash':request_hash_text(request_hash),'timestampMillis':str(self.now-100)},
            'appIntegrity':{'appRecognitionVerdict':'PLAY_RECOGNIZED','packageName':'org.example.wallet','versionCode':'28','certificateSha256Digest':[request_hash_text(b'\x22'*32)]},
            'deviceIntegrity':{'deviceRecognitionVerdict':['MEETS_DEVICE_INTEGRITY']},
            'accountDetails':{'appLicensingVerdict':'LICENSED'}}},separators=(',',':')).encode()

    def rows(self):
        with closing(sqlite3.connect(self.path)) as connection:
            return connection.execute('SELECT operation_id,google_original,google_verified_at_ms,signing_request,certificate FROM ordinary_app_attempts').fetchall()

    def test_actual_interval_refuses_future_not_before_and_upper_expiry(self):
        # Genuine public challenge/P256/Ed fixture; this interval data grants no Native owner.
        for lower,upper in ((self.subject.issued_at_ms-1,self.subject.issued_at_ms+1),
                            (self.subject.expires_at_ms-1,self.subject.expires_at_ms)):
            self.provider._clock=lambda:NativeTimeInterval(lower,upper)
            with self.subTest(lower=lower,upper=upper),self.assertRaises(AttestationRejected):
                self.provider.prepare_raw(self.request.raw_request(),fresh=True)

    def test_retained_raw_encoder_fd_uses_only_dedicated_purpose_and_closes_duplicate(self):
        from iroha_app_attestation.ordinary_raw_admission import signing_request
        from iroha_app_attestation.issuance import encode_raw_admission_with_iroha_fd
        source = os.open(self.openssl, os.O_RDONLY)
        held = CanonicalRawAppAdmissionEncoder(encoder_fd=source,
            encoder_sha256=hashlib.sha256(self.openssl.read_bytes()).digest(),
            authority_key_fd=self.seed_fd, authority_public_key=self.public)
        os.close(source)
        duplicate = held._encoder_fd
        try:
            evidence = self.provider.prepare_raw(self.request.raw_request(), fresh=True)
            request = signing_request(evidence)
            with patch('iroha_app_attestation.ordinary_issuance.encode_raw_admission_with_iroha',
                       side_effect=AssertionError('path encoder must not run')), \
                 patch('iroha_app_attestation.ordinary_issuance.encode_raw_admission_with_iroha_fd',
                       side_effect=self.raw_encoder_original) as child:
                self.assertEqual(held._encode(evidence, request), self.raw_original)
                self.assertEqual(child.call_args.args[1], duplicate)
            with patch('iroha_app_attestation.issuance._encode_with_iroha_raw') as child:
                for malformed in (b'KOAC\x01' + request[5:], request[:-1], request + b'\x00'):
                    with self.assertRaises(AttestationRejected):
                        encode_raw_admission_with_iroha_fd(malformed, duplicate, held._encoder_sha, held._fd)
                child.assert_not_called()
        finally:
            held.close()
        with self.assertRaises(OSError):
            os.fstat(duplicate)

    def test_raw_provider_accepts_original_legacy_tee_only_under_selected_policy_and_revocation(self):
        selected = OrdinaryPlatformEvidenceChallenge(self.subject, bytes(32))
        for version, keymaster in ((2, 3), (3, 4), (4, 41)):
            with self.subTest(version=version, keymaster=keymaster):
                leaf, root = self.fixture.sign('1.3.6.1.4.1.11129.2.1.17',
                    keymint_description(selected, 'org.example.wallet', 28, b'\x22'*32,
                        attestation_version=version, keymaster_version=keymaster,
                        security_level=1, keymint_security_level=1,
                        verified_boot_hash=None if version == 2 else b'\x52'*32))
                self.raw = encode_android_chain([leaf, root])
                self.now = int(time.time()*1000)
                request = replace(self.request, raw_attestation=self.raw).raw_request()
                checked = self.provider.prepare_raw(request, fresh=True)
                self.assertEqual(checked.raw_proof.android_security_level, 1)
                self.assertEqual(checked.raw_proof.attested_public_key_sec1, self.point)
                self.assertEqual(checked.raw_proof.evidence_sha256, hashlib.sha256(self.raw).digest())
                strongbox = replace(self.platform, allowed_security_levels=frozenset({2}))
                with self.assertRaises(AttestationRejected):
                    self.make_provider(replace(self.policy, platform_policy=strongbox)).prepare_raw(request, fresh=True)
                self.revocation_clear = False
                with self.assertRaisesRegex(AttestationRejected, 'revocation'):
                    self.provider.prepare_raw(request, fresh=True)
                self.revocation_clear = True

    def test_raw_provider_requires_full_original_without_possession_or_final_credential(self):
        raw=self.provider.prepare_raw(self.request.raw_request(),fresh=True)
        self.assertEqual(raw.raw_proof.attested_public_key_sec1,self.point)
        self.assertEqual(raw.raw_proof.evidence_sha256,hashlib.sha256(self.raw).digest())
        self.assertEqual(self.raw_rows()[0][1],self.raw_original)
        self.assertEqual(len(self.raw_original),314)
        self.assertEqual(self.raw_original[67:69],bytes([1,2]))
        with closing(sqlite3.connect(self.path)) as connection:
            self.assertEqual(connection.execute('SELECT * FROM ordinary_possession_originals').fetchall(),[])
        self.revocation_clear=False
        with self.assertRaises(AttestationRejected):self.provider.prepare_raw(self.request.raw_request(),fresh=True)

    def test_final_credential_without_retained_raw_admission_stops_before_google_or_encoder(self):
        empty=self.new_issuer()
        with (patch('iroha_app_attestation.play_integrity.urllib.request.build_opener') as network,
              patch('iroha_app_attestation.ordinary_issuance.encode_ordinary_with_iroha') as encoder):
            with self.assertRaisesRegex(AttestationRejected,'raw admission must precede'):
                empty.issue(self.request)
            network.assert_not_called();encoder.assert_not_called()
        self.assertEqual(self.raw_rows(empty._path),[])

    def test_raw_signer_crash_commits_original_input_and_recovers_without_renewal(self):
        issuer=self.new_issuer(); request=self.request.raw_request()
        with patch('iroha_app_attestation.ordinary_issuance.encode_raw_admission_with_iroha',
                   side_effect=RuntimeError('raw signer interrupted')) as encoder:
            with self.assertRaisesRegex(RuntimeError,'raw signer interrupted'):issuer.accept_raw(request)
            original_input=encoder.call_args.args[0]
        self.assertEqual(self.raw_rows(issuer._path)[0][0],original_input)
        self.assertIsNone(self.raw_rows(issuer._path)[0][1])
        self.now+=10
        with patch('iroha_app_attestation.ordinary_issuance.encode_raw_admission_with_iroha',
                   side_effect=self.raw_encoder_original) as encoder:
            retained=self.new_issuer().accept_raw(request)
            self.assertEqual(encoder.call_args.args[0],original_input)
        self.now=self.subject.expires_at_ms+1000
        with patch('iroha_app_attestation.ordinary_issuance.encode_raw_admission_with_iroha') as encoder:
            self.assertEqual(issuer.accept_raw(replace(request,operation='recover')),retained)
            encoder.assert_not_called()
        # Returned historical bytes retain the expired original C interval.
        self.assertEqual(int.from_bytes(retained[234:242],'little'),self.subject.issued_at_ms)
        self.assertEqual(int.from_bytes(retained[242:250],'little'),self.subject.expires_at_ms)

    def test_raw_complete_C_join_rejects_epoch_financial_suite_and_trust_substitutions(self):
        from iroha_app_attestation.ordinary_raw_admission import authenticate_transport
        checked=self.provider.prepare_raw(self.request.raw_request(),fresh=True)
        for change in ({'hardware_epoch':23}, {'financial_authority_commitment':b'\x91'*32},
                       {'suite_id':b'\x92'*32}, {'trust_policy_digest':b'\x93'*32}):
            changed=replace(self.subject,**change)
            # The exact existing signed314 raw transport cannot authenticate
            # against a different full C, even when E's eleven fields overlap.
            with self.assertRaisesRegex(AttestationRejected,'transport differs'):
                authenticate_transport(self.raw_original,replace(checked,challenge=changed),
                                       self.openssl,fresh=True)
            request=replace(self.request.raw_request(),signed_preparation=self.sign_subject(changed),
                            operation_id=changed.attestation_challenge())
            with patch('iroha_app_attestation.ordinary_issuance.encode_raw_admission_with_iroha') as encoder:
                with self.assertRaises(AttestationRejected):self.issuer.accept_raw(request)
                encoder.assert_not_called()
        self.assertEqual(len(self.raw_rows()),1)

    def test_raw_encoder_wrong_body_or_signature_never_publishes(self):
        issuer=self.new_issuer(); request=self.request.raw_request()
        original=self.raw_original
        for bad in (original[:3]+bytes([original[3]^1])+original[4:],original[:-1]+bytes([original[-1]^1])):
            with patch('iroha_app_attestation.ordinary_issuance.encode_raw_admission_with_iroha',return_value=bad):
                with self.assertRaises(AttestationRejected):issuer.accept_raw(request)
            self.assertIsNone(self.raw_rows(issuer._path)[0][1])

    def test_retained_raw_corruption_blocks_final_issuance_and_possession_retention(self):
        bad=self.raw_original[:-1]+bytes([self.raw_original[-1]^1])
        with closing(sqlite3.connect(self.path)) as connection:
            connection.execute('UPDATE ordinary_raw_attempts SET admission=?,admission_sha256=?',
                (bad,hashlib.sha256(bad).digest()));connection.commit()
        with (patch('iroha_app_attestation.play_integrity.urllib.request.build_opener') as network,
              patch('iroha_app_attestation.ordinary_issuance.encode_ordinary_with_iroha') as encoder):
            with self.assertRaisesRegex(AttestationRejected,'issuer signature rejected'):self.issuer.issue(self.request)
            network.assert_not_called();encoder.assert_not_called()
        with closing(sqlite3.connect(self.path)) as connection:
            self.assertEqual(connection.execute('SELECT * FROM ordinary_possession_originals').fetchall(),[])

    def test_real_preparation_attestation_and_possession_required_before_reservation(self):
        evidence = self.provider.prepare(self.request,fresh=True)
        self.assertEqual(evidence.raw_proof.attested_public_key_sec1,self.point)
        self.assertGreater(self.status_checks,0)
        for substituted in (replace(self.request,operation_id=self.subject.enrollment_id),
            replace(self.request,signed_preparation=self.request.signed_preparation[:-1]+bytes([self.request.signed_preparation[-1]^1])),
            replace(self.request,attested_public_key_sec1=b'\x04'+b'\x44'*64),
            replace(self.request,raw_possession=self.pop[:-1]+bytes([self.pop[-1]^1]))):
            with (patch('iroha_app_attestation.ordinary_issuance.encode_ordinary_with_iroha') as encoder,
                 patch('iroha_app_attestation.play_integrity.urllib.request.build_opener') as network):
                with self.assertRaises(AttestationRejected): self.issuer.issue(substituted)
                encoder.assert_not_called(); network.assert_not_called()
        self.assertEqual(self.rows(),[])

    def test_actual_google_original_saved_once_before_one_mocked_encoder_publication(self):
        def encode(request,*args):
            row=self.rows()[0]
            self.assertEqual(row[1],self.google_body)
            self.assertEqual(row[3],request)
            self.assertIsNone(row[4])
            with closing(sqlite3.connect(self.path)) as connection:
                e=connection.execute('SELECT raw_admission_sha256,possession_original,checked_apple_counter FROM ordinary_possession_originals').fetchone()
            self.assertEqual(e[0],hashlib.sha256(self.raw_original).digest())
            self.assertIn(self.pop,e[1]);self.assertEqual(e[2],0)
            self.assertEqual(len(request),896)
            self.assertEqual(request[831:],self.policy.circuit_issuer_public_key)
            return b'public mocked output, deliberately not canonical Norito'
        with (patch('iroha_app_attestation.play_integrity.urllib.request.build_opener') as network,
             patch('iroha_app_attestation.ordinary_issuance.encode_ordinary_with_iroha',side_effect=encode) as encoder):
            network.return_value.open.return_value=Response(self.google_body)
            result=self.issuer.issue(self.request)
            self.now+=1000
            self.assertEqual(self.issuer.issue(self.request),result)
            self.now=self.subject.expires_at_ms+1000
            recover=replace(self.request,operation='recover')
            self.assertEqual(self.issuer.issue(recover),result)
            self.assertEqual(network.return_value.open.call_count,1)
            self.assertEqual(encoder.call_count,1)
            request=encoder.call_args.args[0]
            self.assertEqual(int.from_bytes(request[5+669:5+677],'little')-int.from_bytes(request[5+661:5+669],'little'),3600000)

    def test_restart_after_signer_failure_reuses_retained_google_and_exact_signing_input(self):
        with (patch('iroha_app_attestation.play_integrity.urllib.request.build_opener') as network,
             patch('iroha_app_attestation.ordinary_issuance.encode_ordinary_with_iroha',side_effect=RuntimeError('simulated signer crash')) as encoder):
            network.return_value.open.return_value=Response(self.google_body)
            with self.assertRaisesRegex(RuntimeError,'simulated signer crash'):self.issuer.issue(self.request)
            original_input=encoder.call_args.args[0]
            self.assertEqual(network.return_value.open.call_count,1)
        self.now+=1000
        restarted=DurableOrdinaryCredentialIssuer(path=self.path,provider=self.provider,
            encoder=self.encoder,raw_encoder=self.raw_encoder)
        with (patch('iroha_app_attestation.play_integrity.urllib.request.build_opener') as network,
             patch('iroha_app_attestation.ordinary_issuance.encode_ordinary_with_iroha',return_value=b'mocked-recovered-result') as encoder):
            self.assertEqual(restarted.issue(self.request),b'mocked-recovered-result')
            network.assert_not_called();self.assertEqual(encoder.call_args.args[0],original_input)

    def test_recover_missing_result_and_expired_attempt_never_decode_or_sign(self):
        with (patch('iroha_app_attestation.play_integrity.urllib.request.build_opener') as network,
             patch('iroha_app_attestation.ordinary_issuance.encode_ordinary_with_iroha') as encoder):
            with self.assertRaisesRegex(AttestationRejected,'not available for recovery'):
                self.issuer.issue(replace(self.request,operation='recover'))
            self.now=self.subject.expires_at_ms
            with self.assertRaisesRegex(AttestationRejected,'expired'):
                self.issuer.issue(self.request)
            self.assertEqual(self.rows(),[]);network.assert_not_called();encoder.assert_not_called()

    def test_google_outage_after_reservation_is_retryable_and_the_same_attempt_then_issues(self):
        outage=[OSError('synthetic OAuth outage')]
        def access():
            if outage:raise outage[0]
            return 'synthetic-service-access-token'
        provider=GovernedOrdinaryEvidenceProvider(policies=(self.policy,),
            trusted_time_interval=lambda:NativeTimeInterval(self.now,self.now),recheck_native_policy=self.recheck,
            openssl_path=self.openssl,play_integrity=GooglePlayIntegrityVerifier(access))
        issuer=DurableOrdinaryCredentialIssuer(path=self.path,provider=provider,encoder=self.encoder,raw_encoder=self.raw_encoder)
        owner=object();service=OrdinaryCredentialService(issuer=issuer,authorize_core_call=lambda offered:offered is owner)
        r=self.request;encode=lambda value:base64.b64encode(value).decode()
        body=json.dumps({'schema':SCHEMA,'operation':r.operation,'operation_id':r.operation_id.hex(),
            'signed_preparation_base64':encode(r.signed_preparation),
            'attested_public_key_sec1_base64':encode(r.attested_public_key_sec1),
            'raw_attestation_base64':encode(r.raw_attestation),
            'app_possession':{'platform':'android_keystore','signature_der_base64':encode(r.raw_possession)},
            'play_integrity_token':r.play_integrity_token}).encode()
        handle=lambda:service.handle(method='POST',path=PATH,body=body,content_type='application/json',transport_context=owner)
        unavailable=(503,b'{"error":"issuer_unavailable"}')
        with (patch('iroha_app_attestation.play_integrity.urllib.request.build_opener') as network,
             patch('iroha_app_attestation.ordinary_issuance.encode_ordinary_with_iroha',return_value=b'mocked-result') as encoder):
            network.return_value.open.return_value=Response(self.google_body)
            self.assertEqual(handle(),unavailable)
            network.return_value.open.assert_not_called()
            outage.clear()
            for failure in (urllib.error.HTTPError(Response.url,503,'unavailable',{},None),
                            urllib.error.URLError('offline')):
                network.return_value.open.side_effect=failure
                with self.subTest(failure=type(failure).__name__):self.assertEqual(handle(),unavailable)
            # Only Google's own refusal of the token is a rejected credential.
            network.return_value.open.side_effect=urllib.error.HTTPError(Response.url,400,'INVALID_ARGUMENT',{},None)
            self.assertEqual(handle(),(409,b'{"error":"credential_rejected"}'))
            encoder.assert_not_called()
            row=self.rows()[0];self.assertIsNone(row[1]);self.assertIsNone(row[3])
            network.return_value.open.side_effect=None
            status,result=handle()
        self.assertEqual(status,200)
        self.assertEqual(base64.b64decode(json.loads(result)['certificate_base64']),b'mocked-result')
        self.assertEqual(self.rows()[0][1],self.google_body);self.assertEqual(encoder.call_count,1)

    def test_failed_verdict_reserves_attempt_but_never_publishes_signing_input(self):
        value=json.loads(self.google_body);value['tokenPayloadExternal']['accountDetails']['appLicensingVerdict']='UNLICENSED'
        with (patch('iroha_app_attestation.play_integrity.urllib.request.build_opener') as network,
             patch('iroha_app_attestation.ordinary_issuance.encode_ordinary_with_iroha') as encoder):
            network.return_value.open.return_value=Response(json.dumps(value).encode())
            with self.assertRaises(AttestationRejected):self.issuer.issue(self.request)
            row=self.rows()[0];self.assertIsNone(row[1]);self.assertIsNone(row[3]);self.assertIsNone(row[4]);encoder.assert_not_called()

    def test_current_policy_revocation_conflict_and_google_corruption_block_recovery(self):
        with (patch('iroha_app_attestation.play_integrity.urllib.request.build_opener') as network,
             patch('iroha_app_attestation.ordinary_issuance.encode_ordinary_with_iroha',return_value=b'mocked-result')):
            network.return_value.open.return_value=Response(self.google_body);self.issuer.issue(self.request)
        with (patch('iroha_app_attestation.play_integrity.urllib.request.build_opener') as network,
             patch('iroha_app_attestation.ordinary_issuance.encode_ordinary_with_iroha') as encoder):
            self.native_current=False
            with self.assertRaises(AttestationRejected):self.issuer.issue(self.request)
            self.native_current=True;self.revocation_clear=False
            with self.assertRaises(AttestationRejected):self.issuer.issue(self.request)
            self.revocation_clear=True
            with self.assertRaisesRegex(AttestationRejected,'conflicting'):
                self.issuer.issue(replace(self.request,play_integrity_token='different-opaque-token'))
            with closing(sqlite3.connect(self.path)) as connection:
                connection.execute("UPDATE ordinary_app_attempts SET google_original=?",(b'corrupt',))
                connection.commit()
            with self.assertRaisesRegex(AttestationRejected,'corrupt ordinary Google'):
                self.issuer.issue(replace(self.request,operation='recover'))
            network.assert_not_called();encoder.assert_not_called()

    def test_stale_retained_google_cannot_be_redecoded_to_retry_new_signing(self):
        with (patch('iroha_app_attestation.play_integrity.urllib.request.build_opener') as network,
             patch('iroha_app_attestation.ordinary_issuance.encode_ordinary_with_iroha',side_effect=RuntimeError('crash'))):
            network.return_value.open.return_value=Response(self.google_body)
            with self.assertRaises(RuntimeError):self.issuer.issue(self.request)
        self.now+=6000
        with (patch('iroha_app_attestation.play_integrity.urllib.request.build_opener') as network,
             patch('iroha_app_attestation.ordinary_issuance.encode_ordinary_with_iroha') as encoder):
            with self.assertRaisesRegex(AttestationRejected,'stale'):self.issuer.issue(self.request)
            network.assert_not_called();encoder.assert_not_called()


if __name__=='__main__':unittest.main()
