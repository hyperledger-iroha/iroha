"""Actual crypto over synthetic Iroha ordinary enrollment subjects."""
import hashlib
import shutil
import subprocess
import tempfile
import unittest
from dataclasses import replace
from pathlib import Path
from iroha_app_attestation.attestation import AttestationRejected, RawPlatformProof, device_key_reference, encode_android_chain, verify_android_raw
from iroha_app_attestation.ordinary_enrollment import (
    CHALLENGE_BODY_BYTES, CHALLENGE_TRANSPORT_BYTES, EVIDENCE_DOMAIN,
    OrdinaryEnrollmentChallenge, OrdinaryPlatformEvidenceChallenge,
    authenticate_challenge_transport, credential_signing_request,
    decode_challenge_transport, verify_enrollment_possession,
)
from iroha_app_attestation.play_integrity import PlayIntegrityPolicy, _verify_google_payload, request_hash_text
from test_synthetic_platform_evidence import SignedEnvelope, keymint_description, cbor


def challenge(platform=1):
    return OrdinaryEnrollmentChallenge(platform, *(bytes([i])*32 for i in range(1,14)), 21, 22, 1000, 121000)


class OrdinaryEnrollmentTests(unittest.TestCase):
    def setUp(self):
        executable=shutil.which('openssl')
        if not executable:self.skipTest('OpenSSL CLI unavailable')
        self.openssl=Path(executable).resolve()
        self.temporary=tempfile.TemporaryDirectory(); self.addCleanup(self.temporary.cleanup)
        self.directory=Path(self.temporary.name)
        # Known deterministic isolated test seed; not an installed authority.
        (self.directory/'issuer.der').write_bytes(bytes.fromhex('302e020100300506032b657004220420')+bytes([73])*32)
        self.openssl_run('pkey','-inform','DER','-in','issuer.der','-pubout','-outform','DER','-out','public.der')
        self.public=(self.directory/'public.der').read_bytes()[-32:]

    def openssl_run(self,*arguments):
        subprocess.run([str(self.openssl),*arguments],cwd=self.directory,capture_output=True,check=True)

    def signed(self,subject):
        (self.directory/'message').write_bytes(subject.signing_bytes())
        self.openssl_run('pkeyutl','-sign','-keyform','DER','-inkey','issuer.der','-rawin','-in','message','-out','signature')
        return subject.signing_bytes()[-CHALLENGE_BODY_BYTES:]+(self.directory/'signature').read_bytes()

    def android(self,level=2):
        fixture=SignedEnvelope(self.directory,self.openssl)
        selected=OrdinaryPlatformEvidenceChallenge(challenge(),bytes(32))
        leaf,root=fixture.sign('1.3.6.1.4.1.11129.2.1.17',keymint_description(selected,'org.example.wallet',28,b'\x22'*32,security_level=level,keymint_security_level=level))
        raw=encode_android_chain([leaf,root])
        import time
        proof=verify_android_raw([leaf,root],selected,'org.example.wallet',28,b'\x22'*32,root,hashlib.sha256(root).digest(),int(time.time()*1000),self.openssl,allowed_security_levels=frozenset({1,2}))
        message=challenge().possession_message(hashlib.sha256(proof.attested_public_key_sec1).digest())
        (self.directory/'possession-message').write_bytes(message)
        self.openssl_run('dgst','-sha256','-sign','leaf.key','-out','possession-signature','possession-message')
        pop=(self.directory/'possession-signature').read_bytes()
        possession=verify_enrollment_possession(challenge(),proof,raw,pop,self.openssl,apple_app_id=None)
        return proof,possession,raw,pop

    def request(self,proof,possession,subject=None,policy=None,integrity=None):
        return credential_signing_request(subject or challenge(),proof,possession,b'\x22'*32,b'\x23'*32,self.public,
                1500,61500,60000,121000,frozenset({1,2}) if proof.platform=='android_keymint' else frozenset(),policy,integrity)

    def test_actual_core_signature_binds_every_original_selector_and_signed_epoch(self):
        selected=challenge(); original=self.signed(selected)
        self.assertEqual(len(original),CHALLENGE_TRANSPORT_BYTES)
        self.assertEqual(decode_challenge_transport(original),selected)
        self.assertEqual(authenticate_challenge_transport(original,selected,self.public,1500,self.openssl),selected)
        for change in ({'account_binding':b'\x31'*32},{'hardware_epoch':23},{'network_id':b'\x32'*32},{'financial_authority_commitment':b'\x33'*32}):
            substituted=replace(selected,**change)
            with self.subTest(change=change),self.assertRaises(AttestationRejected):authenticate_challenge_transport(self.signed(substituted),selected,self.public,1500,self.openssl)
        tampered=original[:-1]+bytes([original[-1]^1])
        with self.assertRaisesRegex(AttestationRejected,'signature rejected'):authenticate_challenge_transport(tampered,selected,self.public,1500,self.openssl)
        for malformed in (original[:-1],original+b'\0',bytes(507)):
            with self.assertRaises(AttestationRejected):decode_challenge_transport(malformed)
        with self.assertRaises(AttestationRejected):authenticate_challenge_transport(original,selected,self.public,121000,self.openssl)
        authenticate_challenge_transport(original,selected,self.public,121000,self.openssl,fresh=False)

    def test_android_keymint_and_actual_possession_share_exact_native_subject(self):
        proof,possession,raw,pop=self.android()
        expected=hashlib.sha256(EVIDENCE_DOMAIN+len(raw).to_bytes(8,'little')+raw+len(pop).to_bytes(8,'little')+pop).digest()
        self.assertEqual(possession.platform_evidence_digest,expected)
        self.assertEqual(possession.app_attest_counter_floor,0)
        self.assertNotEqual(expected,proof.evidence_sha256)
        for subject,attestation,signature in ((replace(challenge(),hardware_epoch=23),raw,pop),(challenge(),raw+b'\0',pop),(challenge(),raw,pop[:-1]+bytes([pop[-1]^1]))):
            with self.assertRaises(AttestationRejected):verify_enrollment_possession(subject,proof,attestation,signature,self.openssl,apple_app_id=None)

    def test_exact_koac_unsigned_input_preserves_distinct_key_and_financial_roles(self):
        proof,possession,_,_=self.android()
        request=self.request(proof,possession)
        self.assertEqual(len(request),831); self.assertEqual(request[:5],b'KOAC\x01')
        body=request[5:-32]; self.assertEqual(len(body),794); self.assertEqual(body[:4],b'\x01\x00\x01\x02')
        self.assertEqual(body[4+15*32:4+16*32],challenge().financial_authority_commitment)
        self.assertEqual(body[4+16*32:4+17*32],possession.platform_evidence_digest)
        self.assertEqual(request[-32:],self.public); self.assertEqual(body[-113:],bytes(113))
        self.assertNotEqual(challenge().financial_authority_commitment,proof.device_key_reference)
        with self.assertRaises(AttestationRejected):self.request(proof,replace(possession,attested_key_id=b'\x41'*32))
        with self.assertRaises(AttestationRejected):self.request(proof,replace(possession,app_attest_counter_floor=1))

    def test_selected_tee_is_admitted_and_software_or_unselected_level_is_rejected(self):
        proof,possession,_,_=self.android(level=1)
        self.assertEqual(self.request(proof,possession)[8],1)
        with self.assertRaises(AttestationRejected):self.request(replace(proof,android_security_level=0),possession)
        with self.assertRaises(AttestationRejected):credential_signing_request(challenge(),proof,possession,b'\x22'*32,b'\x23'*32,self.public,1500,61500,60000,121000,frozenset({2}),None,None)

    def test_integrity_binding_is_separate_and_matches_actual_preparation_and_key(self):
        proof,possession,_,_=self.android()
        policy=PlayIntegrityPolicy(b'\x51'*32,'org.example.wallet',28,b'\x22'*32,1000,30000,True,True,'MEETS_DEVICE_INTEGRITY')
        request_hash=challenge().play_integrity_request_hash(possession.attested_key_id)
        import json
        value={'tokenPayloadExternal':{'requestDetails':{'requestPackageName':policy.package_name,'requestHash':request_hash_text(request_hash),'timestampMillis':'1400'},
                'appIntegrity':{'packageName':policy.package_name,'versionCode':'28','appRecognitionVerdict':'PLAY_RECOGNIZED','certificateSha256Digest':[request_hash_text(policy.app_signing_certificate_sha256)]},
                'deviceIntegrity':{'deviceRecognitionVerdict':['MEETS_DEVICE_INTEGRITY']},'accountDetails':{'appLicensingVerdict':'LICENSED'}}}
        original=json.dumps(value).encode(); integrity=_verify_google_payload(original,policy,request_hash,1500,b'\x52'*32)
        request=self.request(proof,possession,policy=policy,integrity=integrity);slot=request[5:-32][-113:]
        self.assertEqual(slot[0],1);self.assertEqual(slot[1:33],request_hash);self.assertEqual(slot[33:65],hashlib.sha256(original).digest())
        self.assertEqual(int.from_bytes(slot[-16:-8],'little'),1500);self.assertEqual(int.from_bytes(slot[-8:],'little'),31500)
        with self.assertRaises(AttestationRejected):self.request(proof,possession,policy=policy,integrity=replace(integrity,request_hash=b'\x53'*32))
        with self.assertRaises(AttestationRejected):self.request(proof,possession,policy=None,integrity=integrity)

    def test_actual_apple_assertion_counter_is_independent_and_becomes_credential_floor(self):
        fixture=SignedEnvelope(self.directory,self.openssl); selected=challenge(platform=2)
        raw=b'isolated attestation original'; point=fixture.point; app_id='TEAMID.example.wallet'
        proof=RawPlatformProof(hashlib.sha256(raw).digest(),point,device_key_reference(point),'apple_app_attest')
        message=selected.possession_message(hashlib.sha256(point).digest())
        auth=hashlib.sha256(app_id.encode()).digest()+b'\x40'+(9).to_bytes(4,'big')
        nonce=hashlib.sha256(auth+hashlib.sha256(message).digest()).digest()
        (self.directory/'apple-nonce').write_bytes(nonce)
        self.openssl_run('dgst','-sha256','-sign','leaf.key','-out','apple-signature','apple-nonce')
        pop=cbor({'signature':(self.directory/'apple-signature').read_bytes(),'authenticatorData':auth})
        possession=verify_enrollment_possession(selected,proof,raw,pop,self.openssl,apple_app_id=app_id)
        self.assertEqual(possession.app_attest_counter_floor,9)
        request=self.request(proof,possession,subject=selected);body=request[5:-32]
        self.assertEqual(body[:4],b'\x01\x00\x02\x03');self.assertEqual(int.from_bytes(body[-117:-113],'little'),9)
        with self.assertRaises(AttestationRejected):self.request(proof,replace(possession,app_attest_counter_floor=0),subject=selected)
        with self.assertRaises(AttestationRejected):verify_enrollment_possession(selected,proof,raw,pop,self.openssl,apple_app_id='TEAMID.other.wallet')

if __name__=='__main__':unittest.main()
