"""Synthetic decoder boundary tests; no genuine Google verdict is claimed."""
import hashlib
import json
import unittest
from dataclasses import replace
from unittest.mock import patch
from iroha_app_attestation.attestation import AttestationRejected
from iroha_app_attestation.play_integrity import GooglePlayIntegrityVerifier, MAX_RESPONSE_BYTES, PlayIntegrityPolicy, _verify_google_payload, request_hash_text
HASH = b'\x33'*32
TOKEN = 'opaque.encrypted.token'
TOKEN_SHA = hashlib.sha256(TOKEN.encode()).digest()
POLICY = PlayIntegrityPolicy(b'\x11'*32, 'org.example.app', 28, b'\x22'*32, 1000, 60_000, True, True, 'MEETS_DEVICE_INTEGRITY')
def payload():
    return {'tokenPayloadExternal': {
        'requestDetails': {'requestPackageName': POLICY.package_name, 'requestHash': request_hash_text(HASH), 'timestampMillis': '10000'},
        'appIntegrity': {'appRecognitionVerdict': 'PLAY_RECOGNIZED', 'packageName': POLICY.package_name, 'certificateSha256Digest': [request_hash_text(POLICY.app_signing_certificate_sha256)], 'versionCode': '28'},
        'deviceIntegrity': {'deviceRecognitionVerdict': ['MEETS_DEVICE_INTEGRITY', 'MEETS_STRONG_INTEGRITY']},
        'accountDetails': {'appLicensingVerdict': 'LICENSED'}}}
def encoded(value): return json.dumps(value, separators=(',', ':')).encode()
def verify(value, policy=POLICY, now=10500): return _verify_google_payload(encoded(value), policy, HASH, now, TOKEN_SHA)
class Response:
    status = 200
    headers = {'Content-Type': 'application/json; charset=utf-8'}
    url = f'https://playintegrity.googleapis.com/v1/{POLICY.package_name}:decodeIntegrityToken'
    body = encoded(payload())
    def __enter__(self): return self
    def __exit__(self, *args): pass
    def geturl(self): return self.url
    def read(self, bound):
        self.requested_bound = bound
        return self.body[:bound]
class PlayIntegrityTests(unittest.TestCase):
    def test_matching_enrollment_and_key_hash_keeps_exact_evidence(self):
        value=payload(); proof=verify(value)
        self.assertEqual(proof.request_hash,HASH); self.assertEqual(proof.policy_digest,POLICY.policy_digest)
        self.assertEqual(proof.google_response_sha256,hashlib.sha256(encoded(value)).digest())
        self.assertEqual(proof.device_integrity,('MEETS_DEVICE_INTEGRITY','MEETS_STRONG_INTEGRITY'))
        self.assertEqual(len(request_hash_text(HASH)),43); self.assertNotIn('=',request_hash_text(HASH))
    def test_every_enrollment_identity_and_signer_substitution_is_rejected(self):
        for group,field,replacement in (
            ('requestDetails','requestPackageName','org.other.app'),('requestDetails','requestHash',request_hash_text(b'\x34'*32)),('requestDetails','requestHash',request_hash_text(HASH)+'='),
            ('appIntegrity','packageName','org.other.app'),('appIntegrity','versionCode','27'),('appIntegrity','versionCode','028'),('appIntegrity','versionCode',28),
            ('appIntegrity','certificateSha256Digest',[request_hash_text(b'\x23'*32)]),('appIntegrity','certificateSha256Digest',[request_hash_text(POLICY.app_signing_certificate_sha256)]*2),
            ('appIntegrity','appRecognitionVerdict','UNEVALUATED'),('appIntegrity','appRecognitionVerdict','UNRECOGNIZED_VERSION'),('accountDetails','appLicensingVerdict','UNLICENSED'),('accountDetails','appLicensingVerdict','UNEVALUATED')):
            value=payload(); value['tokenPayloadExternal'][group][field]=replacement
            with self.subTest(field=field,replacement=replacement), self.assertRaises(AttestationRejected): verify(value)
    def test_freshness_rejects_future_stale_and_invalid_time(self):
        for timestamp in ('10501','9499','0','-1','010000',str(1<<64),10000):
            value=payload(); value['tokenPayloadExternal']['requestDetails']['timestampMillis']=timestamp
            with self.subTest(timestamp=timestamp), self.assertRaises(AttestationRejected): verify(value)
        self.assertEqual(verify(payload(),now=11000).timestamp_ms,10000)
        for now in (0,True,-1,1<<64):
            with self.subTest(now=now), self.assertRaises(AttestationRejected): verify(payload(),now=now)
    def test_missing_empty_duplicate_virtual_and_weak_device_verdicts(self):
        for labels in (None,[],['MEETS_BASIC_INTEGRITY'],['MEETS_VIRTUAL_INTEGRITY'],['MEETS_DEVICE_INTEGRITY','MEETS_VIRTUAL_INTEGRITY'],['MEETS_DEVICE_INTEGRITY']*2,['UNKNOWN'],'MEETS_DEVICE_INTEGRITY'):
            value=payload(); value['tokenPayloadExternal']['deviceIntegrity']['deviceRecognitionVerdict']=labels
            with self.subTest(labels=labels), self.assertRaises(AttestationRejected): verify(value)
        value=payload(); value['tokenPayloadExternal']['deviceIntegrity']['deviceRecognitionVerdict']=['MEETS_DEVICE_INTEGRITY']; verify(value)
        with self.assertRaises(AttestationRejected): verify(value,replace(POLICY,minimum_device_integrity='MEETS_STRONG_INTEGRITY'))
    def test_console_testing_response_never_establishes_release_admission(self):
        for details in ({'isTestingResponse':True},{'isTestingResponse':False},{},None):
            value=payload(); value['tokenPayloadExternal']['testingDetails']=details
            with self.subTest(details=details), self.assertRaisesRegex(AttestationRejected,'testing response'): verify(value)
    def test_only_authenticated_policy_can_relax_optional_verdicts(self):
        value=payload(); value['tokenPayloadExternal']['appIntegrity']['appRecognitionVerdict']='UNRECOGNIZED_VERSION'; value['tokenPayloadExternal']['accountDetails']['appLicensingVerdict']='UNLICENSED'
        policy=replace(POLICY,require_play_recognized=False,require_licensed=False); verify(value,policy)
        value['tokenPayloadExternal']['appIntegrity']['certificateSha256Digest']=[request_hash_text(b'\x24'*32)]
        with self.assertRaises(AttestationRejected): verify(value,policy)
    def test_invalid_policy_and_ambiguous_decoder_payload_are_rejected(self):
        for change in ({'policy_digest':b'\0'*32},{'package_name':'../wrong'},{'package_version':True},{'maximum_evidence_age_ms':0},{'maximum_refresh_interval_ms':True},{'require_licensed':1},{'minimum_device_integrity':'MEETS_BASIC_INTEGRITY'}):
            with self.subTest(change=change), self.assertRaises(AttestationRejected): verify(payload(),replace(POLICY,**change))
        for body in (b'{"tokenPayloadExternal":{},"tokenPayloadExternal":{}}',b'{}',b'[]',encoded(payload())+b' trailing',b'NaN',b' '*(MAX_RESPONSE_BYTES+1)):
            with self.subTest(body=body[:100]), self.assertRaises(AttestationRejected): _verify_google_payload(body,POLICY,HASH,10500,TOKEN_SHA)
    def test_public_verifier_uses_only_fixed_google_https_decoder(self):
        response=Response()
        with patch('iroha_app_attestation.play_integrity.urllib.request.build_opener') as build:
            build.return_value.open.return_value=response
            proof=GooglePlayIntegrityVerifier(lambda:'server-oauth').verify(TOKEN,POLICY,HASH,10500)
            request=build.return_value.open.call_args.args[0]
            self.assertEqual(request.full_url,response.url); self.assertEqual(request.get_method(),'POST')
            self.assertEqual(json.loads(request.data),{'integrity_token':TOKEN}); self.assertEqual(request.get_header('Authorization'),'Bearer server-oauth')
            self.assertEqual(build.return_value.open.call_args.kwargs,{'timeout':5}); self.assertEqual(response.requested_bound,MAX_RESPONSE_BYTES+1)
            self.assertEqual(proof.token_sha256,TOKEN_SHA); self.assertEqual(build.call_args.args[0].proxies,{})
    def test_decoder_failure_redirect_media_compression_and_oversize_fail_closed(self):
        for change in ({'status':302},{'url':'https://attacker.example/decoder'},{'headers':{'Content-Type':'text/html'}},{'headers':{'Content-Type':'application/json','Content-Encoding':'gzip'}},{'body':b' '*(MAX_RESPONSE_BYTES+1)}):
            response=Response()
            for key,value in change.items(): setattr(response,key,value)
            with patch('iroha_app_attestation.play_integrity.urllib.request.build_opener') as build:
                build.return_value.open.return_value=response
                with self.subTest(change=list(change)), self.assertRaises(AttestationRejected): GooglePlayIntegrityVerifier(lambda:'server-oauth').verify(TOKEN,POLICY,HASH,10500)
        with patch('iroha_app_attestation.play_integrity.urllib.request.build_opener') as build:
            build.return_value.open.side_effect=OSError('unavailable')
            with self.assertRaisesRegex(AttestationRejected,'unavailable'): GooglePlayIntegrityVerifier(lambda:'server-oauth').verify(TOKEN,POLICY,HASH,10500)
        for token in ('','secret\nheader',None):
            with self.subTest(token=token), self.assertRaises(AttestationRejected): GooglePlayIntegrityVerifier(lambda:token).verify(TOKEN,POLICY,HASH,10500)
        with self.assertRaises(AttestationRejected): GooglePlayIntegrityVerifier(lambda:'server-oauth').verify(payload(),POLICY,HASH,10500)
if __name__=='__main__': unittest.main()
