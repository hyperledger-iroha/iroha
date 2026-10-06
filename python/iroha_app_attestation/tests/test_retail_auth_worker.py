"""Auth-only component tests with ephemeral signed synthetic certificates.

Only HTTPS transport/OAuth custody is isolated. KeyMint DER, pinned X509 chain,
P-256 possession, Google response parsing and SQLite recovery execute current
production primitives. No test supplies Native release or device qualification.
"""
import base64
import hashlib
import importlib.util
import io
import json
import os
import shutil
import sqlite3
import subprocess
import tempfile
import time
import unittest
import zipfile
from pathlib import Path
from unittest.mock import patch

from iroha_app_attestation import retail_auth_worker as auth
from iroha_app_attestation import play_integrity as pi
from iroha_app_attestation import revocation
from iroha_app_attestation.google_oauth import select_google_decoder
from iroha_app_attestation.attestation import (
    ANDROID_KEY_DESCRIPTION_OID, AttestationRejected, VerificationUnavailable,
    children, der_one, primitive,
)


def der(tag, content):
    size = len(content)
    length = bytes([size]) if size < 128 else bytes([0x80 | ((size.bit_length()+7)//8)]) + size.to_bytes((size.bit_length()+7)//8, 'big')
    return tag + length + content


def sequence(*items): return der(b'\x30', b''.join(items))
def integer(value, tag=b'\x02'):
    raw = value.to_bytes(max(1, (value.bit_length()+7)//8), 'big')
    return der(tag, (b'\0' if raw[0] >= 128 else b'') + raw)
def octets(value): return der(b'\x04', value)
def set_of(*items): return der(b'\x31', b''.join(items))
def explicit(number, content):
    if number < 31: return der(bytes([0xa0 | number]), content)
    parts = [number & 127]
    number >>= 7
    while number:
        parts.insert(0, 0x80 | (number & 127)); number >>= 7
    return der(b'\xbf' + bytes(parts), content)


def transcript(now):
    return auth.CHALLENGE_DOMAIN + b''.join(bytes([n])*32 for n in range(1,8)) + (now-1000).to_bytes(8,'little') + (now+120000).to_bytes(8,'little')


class SignedKeyMint:
    """Actual ephemeral OpenSSL signing; the root has no production trust."""
    def __init__(self, directory, openssl):
        self.directory, self.openssl = directory, openssl
        self.run('req','-x509','-newkey','ec','-pkeyopt','ec_paramgen_curve:P-256','-nodes',
                 '-keyout','root.key','-out','root.pem','-days','2','-subj','/CN=Auth Synthetic Root',
                 '-addext','basicConstraints=critical,CA:TRUE','-addext','keyUsage=critical,keyCertSign,cRLSign')
        self.run('req','-newkey','ec','-pkeyopt','ec_paramgen_curve:P-256','-nodes',
                 '-keyout','leaf.key','-out','leaf.csr','-subj','/CN=Auth Synthetic App Key')
        self.run('pkey','-in','leaf.key','-pubout','-outform','DER','-out','leaf.spki')
        self.point = primitive(children(der_one((directory/'leaf.spki').read_bytes()))[1],3)[1:]
        self.run('x509','-in','root.pem','-outform','DER','-out','root.der')
        self.root = (directory/'root.der').read_bytes()

    def run(self, *arguments):
        subprocess.run([str(self.openssl), *arguments],cwd=self.directory,stdin=subprocess.DEVNULL,
                       capture_output=True,check=True,timeout=10)

    def chain(self, challenge, *, version=300, keymaster=300, level=2,
              package='pg.bpng.digitalkina', signer=b'\x32'*32, locked=True, usage=None):
        app = sequence(set_of(sequence(octets(package.encode()),integer(29))),set_of(octets(signer)))
        software = sequence(explicit(709,octets(app)))
        boot = sequence(octets(b'\x51'*32),der(b'\x01',b'\xff' if locked else b'\0'),integer(0,b'\x0a'),
                        *([] if version == 2 else [octets(b'\x52'*32)]))
        tags = [explicit(1,set_of(integer(2))),explicit(2,integer(3)),explicit(3,integer(256)),
                explicit(5,set_of(integer(4))),explicit(10,integer(1))]
        if usage is not None: tags.append(explicit(405,integer(usage)))
        tags += [explicit(702,integer(0)),explicit(704,boot)]
        # Genuine older evidence is accepted without invented missing patch requirements.
        description = sequence(integer(version),integer(level,b'\x0a'),integer(keymaster),integer(level,b'\x0a'),
                               octets(challenge),octets(b''),software,sequence(*tags))
        (self.directory/'extension.cnf').write_text(ANDROID_KEY_DESCRIPTION_OID+'=DER:'+':'.join(f'{n:02X}' for n in description)+'\n')
        self.run('x509','-req','-in','leaf.csr','-CA','root.pem','-CAkey','root.key','-CAcreateserial',
                 '-out','leaf.pem','-days','2','-extfile','extension.cnf')
        self.run('x509','-in','leaf.pem','-outform','DER','-out','leaf.der')
        return [(self.directory/'leaf.der').read_bytes(),self.root]

    def sign(self, message):
        (self.directory/'message').write_bytes(message)
        self.run('dgst','-sha256','-sign','leaf.key','-out','signature.der','message')
        return (self.directory/'signature.der').read_bytes()


class Response:
    def __init__(self, url, body):
        self.url, self.body, self.status = url, body, 200
        self.headers = {'Content-Type':'application/json','Content-Encoding':'identity'}
    def geturl(self): return self.url
    def read(self, bound): return self.body[:bound]
    def __enter__(self): return self
    def __exit__(self,*unused): pass


class SyntheticTransport:
    def __init__(self):
        self.revocations, self.decodes = 0, 0
        self.revoked_serials = []
        self.expected_hash = None
        self.now = None
        self.mutate = lambda payload: None
        self.last_response = None
    def open(self, request, timeout):
        url = request.full_url
        if url == revocation.GOOGLE_STATUS_URL:
            self.revocations += 1
            body = auth.encode({'entries':{format(v,'x'):{'status':'REVOKED'} for v in self.revoked_serials}})
        else:
            self.decodes += 1
            if url != 'https://playintegrity.googleapis.com/v1/pg.bpng.digitalkina:decodeIntegrityToken':
                raise AssertionError('foreign decoder URL')
            if request.get_method() != 'POST' or json.loads(request.data) != {'integrity_token':'opaque.synthetic.token'}:
                raise AssertionError('foreign token original')
            payload = {'requestDetails':{'requestPackageName':'pg.bpng.digitalkina',
                       'requestHash':pi.request_hash_text(self.expected_hash),'timestampMillis':str(self.now)},
                       'appIntegrity':{'appRecognitionVerdict':'PLAY_RECOGNIZED','packageName':'pg.bpng.digitalkina',
                       'versionCode':'29','certificateSha256Digest':[pi.request_hash_text(b'\x32'*32)]},
                       'deviceIntegrity':{'deviceRecognitionVerdict':['MEETS_DEVICE_INTEGRITY']},
                       'accountDetails':{'appLicensingVerdict':'LICENSED'}}
            self.mutate(payload)
            body = auth.encode({'tokenPayloadExternal':payload})
            self.last_response = body
        return Response(url,body)


class AuthComponentTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.openssl = Path(shutil.which('openssl')).resolve(strict=True)
        cls.temporary = tempfile.TemporaryDirectory(prefix='bpng-auth-component-')
        cls.fixture = SignedKeyMint(Path(cls.temporary.name),cls.openssl)
        # The fixture certificates are minted during the test run; keep the bounded
        # synthetic capture later than all of their real notBefore timestamps.
        cls.now = int(time.time()*1000)+60000
    @classmethod
    def tearDownClass(cls): cls.temporary.cleanup()

    def setUp(self):
        self.store_temporary = tempfile.TemporaryDirectory(prefix='bpng-auth-store-')
        self.directory = Path(self.store_temporary.name).resolve()
        self.directory.chmod(0o700)
        self.directory_fd = os.open(self.directory,os.O_RDONLY)
        self.journal = auth.AuthJournal(self.directory,self.directory_fd)
        self.crypto_fd = os.open(self.openssl,os.O_RDONLY)
        owner = self.owner = object.__new__(auth.AuthVerifierOwner)
        owner.config_digest = b'\x11'*32
        owner.openssl = self.openssl
        owner.crypto = auth.HeldPublicFile(self.crypto_fd,auth.sha(self.openssl.read_bytes()),self.openssl)
        owner.root = self.fixture.root
        owner.root_digest = auth.sha(owner.root)
        owner.security_levels = frozenset({1,2})
        owner.pi_policy = pi.PlayIntegrityEnrollmentPolicy(b'\x22'*32,'pg.bpng.digitalkina',29,b'\x32'*32,
                                                           120000,True,True,'MEETS_DEVICE_INTEGRITY')
        owner.current_time = None
        owner.google = pi.GooglePlayIntegrityVerifier(lambda:'synthetic.server.oauth')
        owner.journal = self.journal
        self.transport = SyntheticTransport()
        self.network = patch('urllib.request.build_opener',return_value=self.transport)
        self.network.start()
        self.transcript = transcript(self.now)

    def tearDown(self):
        self.network.stop()
        self.journal.close(); os.close(self.directory_fd); os.close(self.crypto_fd)
        self.store_temporary.cleanup()

    def raw_request(self, chain=None, now=None):
        chain = chain if chain is not None else self.fixture.chain(auth.sha(self.transcript))
        return auth.encode({'phase':'raw','challenge_transcript_base64':auth.text(self.transcript),
                            'certificate_chain_der_base64':[auth.text(v) for v in chain],
                            'trusted_time_ms':self.now if now is None else now})

    def configuration(self):
        return {'schema':auth.CONFIG_SCHEMA,'version':1,'openssl_path':str(self.openssl),
                'openssl_sha256':auth.sha(self.openssl.read_bytes()).hex(),'store_directory':str(self.directory),
                'policy':{'package_name':'pg.bpng.digitalkina','package_version':29,
                    'app_certificate_sha256':(b'\x32'*32).hex(),'root_base64':auth.text(self.fixture.root),
                    'root_sha256':auth.sha(self.fixture.root).hex(),'security_levels':[1,2],
                    'patch_floor_yyyymm':202609,'google_policy_base64':auth.text(b'explicit synthetic policy original'),
                    'google_policy_sha256':(b'\x22'*32).hex(),'maximum_evidence_age_ms':120000,
                    'require_play_recognized':True,'require_licensed':True,'minimum_device_integrity':'MEETS_DEVICE_INTEGRITY'}}

    def test_configuration_has_no_issuer_key_and_preserves_authentication_policy(self):
        calls=[]
        class SyntheticOAuth:
            def __init__(self,**kwargs):calls.append(kwargs)
            def __call__(self):return 'synthetic.server.oauth'
            def close(self):pass
        original=auth.encode(self.configuration())
        with patch.object(auth,'STORE_DIRECTORY',self.directory),patch.object(auth,'GoogleServiceAccountTokenProvider',SyntheticOAuth):
            owner=auth.AuthVerifierOwner(original,directory_fd=self.directory_fd,crypto_fd=self.crypto_fd,oauth_fd=991)
            try:
                self.assertEqual(owner.config_digest,auth.sha(original))
                self.assertEqual(owner.pi_policy.minimum_device_integrity,'MEETS_DEVICE_INTEGRITY')
                self.assertEqual(owner.security_levels,frozenset({1,2}))
                self.assertEqual(calls[0]['credential_fd'],991)
                self.assertEqual(calls[0]['credential_owner_uid'],0)
                self.assertFalse(any('issuer' in key for key in calls[0]))
            finally:owner.close()

    def test_configuration_rejects_extra_boolean_and_software_policy_fields(self):
        offered=self.configuration()
        malformed=[dict(offered,issuer_key='forbidden'),dict(offered,version=True),
                   dict(offered,policy=dict(offered['policy'],security_levels=[0])),
                   dict(offered,policy=dict(offered['policy'],security_levels=[True,2])),
                   dict(offered,policy=dict(offered['policy'],hardware_monotonic_counter=True))]
        with patch.object(auth,'STORE_DIRECTORY',self.directory),patch.object(auth,'GoogleServiceAccountTokenProvider',side_effect=AssertionError('must reject before credential custody')):
            for value in malformed:
                with self.subTest(keys=list(value)),self.assertRaises(AttestationRejected):auth.AuthVerifierOwner(auth.encode(value),directory_fd=self.directory_fd,crypto_fd=self.crypto_fd)

    def test_configuration_rejects_foreign_store_and_changed_crypto_original(self):
        offered=self.configuration()
        with patch.object(auth,'STORE_DIRECTORY',self.directory),patch.object(auth,'GoogleServiceAccountTokenProvider',side_effect=AssertionError('must reject before credential custody')):
            for value in (dict(offered,store_directory=str(self.directory/'other')),dict(offered,openssl_sha256=(b'\x71'*32).hex()),
                          dict(offered,policy=dict(offered['policy'],root_sha256=(b'\x72'*32).hex()))):
                with self.subTest(keys=list(value)),self.assertRaises(AttestationRejected):auth.AuthVerifierOwner(auth.encode(value),directory_fd=self.directory_fd,crypto_fd=self.crypto_fd)

    def test_configuration_rejects_invented_or_malformed_patch_floor(self):
        offered=self.configuration()
        with patch.object(auth,'STORE_DIRECTORY',self.directory),patch.object(auth,'GoogleServiceAccountTokenProvider',side_effect=AssertionError('must reject before credential custody')):
            for value in (True,0,202613,'202609'):
                changed=dict(offered,policy=dict(offered['policy'],patch_floor_yyyymm=value))
                with self.subTest(value=value),self.assertRaises(AttestationRejected):auth.AuthVerifierOwner(auth.encode(changed),directory_fd=self.directory_fd,crypto_fd=self.crypto_fd)

    def test_current_public_oauth_selector_accepts_exact_enrollment_policy(self):
        public={'schema':'iroha.kagemusha.play-integrity-verification-policy.v1','version':1,
                'cloudProject':{'id':'synthetic-auth-project','number':10001},'packageName':'pg.bpng.digitalkina',
                'packageVersion':29,'appSigningCertificateSha256Hex':(b'\x32'*32).hex(),
                'credentialSubject':{'email':'retail-auth-decoder@synthetic-auth-project.iam.gserviceaccount.com','clientId':'123456789'}}
        original=auth.encode(public)
        policy=pi.PlayIntegrityEnrollmentPolicy(auth.sha(original),'pg.bpng.digitalkina',29,b'\x32'*32,
                                                120000,True,True,'MEETS_DEVICE_INTEGRITY')
        selection=select_google_decoder(original,policy)
        self.assertEqual(selection.project_id,'synthetic-auth-project')
        self.assertEqual(selection.service_account_client_id,'123456789')
        with self.assertRaises(AttestationRejected):select_google_decoder(original+b' ',policy)

    def raw_result(self, chain=None):
        original = self.raw_request(chain)
        return original, self.owner.perform(original,'verify-raw')

    def finish_request(self, raw, *, now=None):
        result = auth.exact_json(raw,auth.MAX_ORIGINAL)
        chain = [auth.b64(v,16384) for v in result['original_chain_base64']]
        message = auth.possession_message(self.transcript,chain,self.fixture.point)
        signature = self.fixture.sign(message)
        self.transport.now = self.now if now is None else now
        self.transport.expected_hash = auth.integrity_request_hash(self.transcript,raw,message,signature)
        original = auth.encode({'phase':'finish','challenge_transcript_base64':auth.text(self.transcript),
                               'raw_verifier_original_base64':auth.text(raw),'possession_message_base64':auth.text(message),
                               'possession_signature_der_base64':auth.text(signature),'play_integrity_token':'opaque.synthetic.token',
                               'trusted_time_ms':self.transport.now})
        return original

    def test_challenge_exact_domain_fields_endianness(self):
        expected = b'BPNG.FIRST_DEVICE.AUTH.CHALLENGE.V1\0' + b''.join(bytes([n])*32 for n in range(1,8))
        expected += (self.now-1000).to_bytes(8,'little')+(self.now+120000).to_bytes(8,'little')
        self.assertEqual(self.transcript,expected)
        self.assertEqual(auth.challenge(expected),(hashlib.sha256(expected).digest(),self.now-1000,self.now+120000))
        for n in range(7):
            changed = bytearray(expected); changed[len(auth.CHALLENGE_DOMAIN)+n*32] ^= 1
            self.assertNotEqual(auth.challenge(bytes(changed))[0],auth.challenge(expected)[0])

    def test_challenge_rejects_noncanonical_and_expired_layouts(self):
        for original in (self.transcript[:-1],self.transcript+b'\0',b'X'+self.transcript[1:],
                         self.transcript[:-8]+(self.now+700000).to_bytes(8,'little'),
                         self.transcript[:-16]+b'\0'*16):
            with self.subTest(original=original[:8]),self.assertRaises(AttestationRejected): auth.challenge(original)

    def test_challenge_requires_each_owner_operation_and_nonce_field(self):
        for n in range(7):
            original = bytearray(self.transcript)
            original[len(auth.CHALLENGE_DOMAIN)+n*32:len(auth.CHALLENGE_DOMAIN)+(n+1)*32] = b'\0'*32
            with self.subTest(field=n),self.assertRaises(AttestationRejected): auth.challenge(bytes(original))
        same = bytearray(self.transcript); start=len(auth.CHALLENGE_DOMAIN)
        same[start+3*32:start+4*32]=same[start+2*32:start+3*32]
        with self.assertRaises(AttestationRejected): auth.challenge(bytes(same))

    def test_chain_hash_frames_original_count_order_and_lengths(self):
        chain=[b'abc',b'defg']
        expected=hashlib.sha256(b'BPNG.FIRST_DEVICE.AUTH.CHAIN.V1\0'+(2).to_bytes(4,'little')+
                                (3).to_bytes(4,'little')+b'abc'+(4).to_bytes(4,'little')+b'defg').digest()
        self.assertEqual(auth.chain_digest(chain),expected)
        self.assertNotEqual(expected,auth.chain_digest(chain[::-1]))
        self.assertNotEqual(expected,auth.chain_digest([b'ab',b'cdefg']))

    def test_cross_language_challenge_and_chain_goldens(self):
        original=auth.CHALLENGE_DOMAIN+b''.join(bytes([n])*32 for n in range(1,8))+(1000).to_bytes(8,'little')+(2000).to_bytes(8,'little')
        self.assertEqual(len(original),276)
        self.assertEqual(auth.challenge(original)[0].hex(),'af89ead9caf2a270c0ab37891f2ca0c214f183e5511fc38351ec6ebfff578aab')
        self.assertEqual(auth.chain_digest([b'\x01\x02\x03',b'\x04\x05']).hex(),'67efd007782515431b15150d20b6899893e492361c824058169c05bd88badbd5')

    def test_chain_rejects_unbounded_empty_and_missing_originals(self):
        for chain in ([b'x'],[b'',b'x'],[b'x']*9,[b'x'*16385,b'y'],(b'x',b'y')):
            with self.subTest(chain_type=type(chain).__name__),self.assertRaises(AttestationRejected): auth.chain_digest(chain)

    def test_integrity_hash_binds_all_four_exact_originals(self):
        items=(self.transcript,b'raw original',b'possession original',b'platform DER original')
        expected=hashlib.sha256(b'BPNG.FIRST_DEVICE.AUTH.INTEGRITY.V1\0'+b''.join(len(v).to_bytes(4,'little')+v for v in items)).digest()
        self.assertEqual(auth.integrity_request_hash(*items),expected)
        self.assertEqual(len(pi.request_hash_text(expected)),43)
        self.assertNotIn('=',pi.request_hash_text(expected))
        for n in range(4):
            changed=list(items);changed[n]+=b'X'
            self.assertNotEqual(auth.integrity_request_hash(*changed),expected)

    def test_json_and_base64_are_closed_and_canonical(self):
        for value in (b'{"x":1,"x":2}',b'{"x":NaN}',b'[]',b'{} trailing',b'\xff'):
            with self.subTest(value=value),self.assertRaises(AttestationRejected): auth.exact_json(value,100)
        for value in ('','YQ','YQ===','YQ==\n','YR==','_A=='):
            with self.subTest(value=value),self.assertRaises(AttestationRejected): auth.b64(value,16)
        self.assertEqual(auth.b64('YQ==',16),b'a')

    def test_actual_p256_possession_signature_and_unchanged_der(self):
        chain=self.fixture.chain(auth.sha(self.transcript))
        message=auth.possession_message(self.transcript,chain,self.fixture.point)
        signature=self.fixture.sign(message)
        auth.verify_possession(self.openssl,self.fixture.point,message,message,signature)
        self.assertEqual(message,b'BPNG.FIRST_DEVICE.AUTH.POSSESSION.V1\0'+auth.sha(self.transcript)+auth.chain_digest(chain)+self.fixture.point)
        with self.assertRaises(AttestationRejected): auth.verify_possession(self.openssl,self.fixture.point,message,message+b'X',signature)
        changed=bytearray(signature);changed[-1]^=1
        with self.assertRaises(AttestationRejected): auth.verify_possession(self.openssl,self.fixture.point,message,message,bytes(changed))

    def test_possession_rejects_nonminimal_der_and_foreign_key(self):
        message=b'auth original'
        signature=self.fixture.sign(message)
        malformed=b'\x30\x81'+bytes([len(signature)-2])+signature[2:]
        with self.assertRaises(AttestationRejected): auth.verify_possession(self.openssl,self.fixture.point,message,message,malformed)
        with self.assertRaises(AttestationRejected): auth.verify_possession(self.openssl,b'\x04'+b'\0'*64,message,message,signature)

    def test_signed_strongbox_raw_is_stored_before_result_returns(self):
        request, raw=self.raw_result()
        result=auth.exact_json(raw,auth.MAX_ORIGINAL)
        self.assertEqual(result['security_level'],2)
        self.assertEqual(auth.b64(result['app_public_key_sec1_base64'],65),self.fixture.point)
        self.assertEqual(result['raw_request_sha256'],auth.sha(request).hex())
        self.assertEqual(self.journal.cached('raw',auth.sha(self.transcript),request),raw)
        self.assertEqual((self.transport.revocations,self.transport.decodes),(1,0))

    def test_signed_api26_keymaster_tee_without_patch_tags_remains_supported(self):
        chain=self.fixture.chain(auth.sha(self.transcript),version=2,keymaster=3,level=1)
        _,raw=self.raw_result(chain)
        self.assertEqual(auth.exact_json(raw,auth.MAX_ORIGINAL)['security_level'],1)

    def test_wrong_attested_app_is_rejected_and_reservation_never_reexecutes(self):
        original=self.raw_request(self.fixture.chain(auth.sha(self.transcript),package='foreign.application'))
        with self.assertRaises(AttestationRejected): self.owner.perform(original,'verify-raw')
        self.assertIsNone(self.owner.perform(original,'recover-raw'))
        with self.assertRaisesRegex(AttestationRejected,'attempt_consumed'): self.owner.perform(original,'verify-raw')
        self.assertEqual(self.transport.revocations,0)

    def test_wrong_attestation_challenge_is_rejected_before_revocation(self):
        original=self.raw_request(self.fixture.chain(b'\x77'*32))
        with self.assertRaises(AttestationRejected): self.owner.perform(original,'verify-raw')
        self.assertEqual(self.transport.revocations,0)

    def test_software_or_usage_limited_keys_are_rejected(self):
        for kwargs in ({'level':0},{'usage':1},{'locked':False},{'signer':b'\x33'*32}):
            chain=self.fixture.chain(auth.sha(self.transcript),**kwargs)
            request=self.raw_request(chain)
            with self.subTest(kwargs=kwargs),self.assertRaises(AttestationRejected): self.owner.raw(auth.exact_json(request,auth.MAX_ORIGINAL),self.transcript,auth.sha(self.transcript),self.now,request)
        self.assertEqual(self.transport.revocations,0)

    def test_changed_x509_bytes_and_root_are_rejected(self):
        chain=self.fixture.chain(auth.sha(self.transcript))
        changed=bytearray(chain[0]);changed[-1]^=1
        request=self.raw_request([bytes(changed),chain[1]])
        with self.assertRaises(AttestationRejected): self.owner.perform(request,'verify-raw')

    def test_actual_revocation_parser_rejects_signed_certificate(self):
        chain=self.fixture.chain(auth.sha(self.transcript))
        self.transport.revoked_serials=[revocation.certificate_serial(chain[0])]
        original=self.raw_request(chain)
        with self.assertRaisesRegex(AttestationRejected,'revoked'): self.owner.perform(original,'verify-raw')
        self.assertEqual(self.transport.revocations,1)

    def test_raw_revocation_failure_is_reserved_unknown_without_reexecution(self):
        original=self.raw_request()
        with patch.object(self.transport,'open',side_effect=OSError('synthetic transport failure')):
            with self.assertRaises(VerificationUnavailable): self.owner.perform(original,'verify-raw')
        self.assertIsNone(self.owner.perform(original,'recover-raw'))
        with self.assertRaisesRegex(AttestationRejected,'attempt_consumed'): self.owner.perform(original,'verify-raw')

    def test_finish_executes_real_possession_and_google_parser_retains_original_response(self):
        _,raw=self.raw_result()
        request=self.finish_request(raw)
        result=self.owner.perform(request,'verify-finish')
        value=auth.exact_json(result,auth.MAX_ORIGINAL)
        self.assertEqual(value['integrity_request_hash'],self.transport.expected_hash.hex())
        self.assertEqual(auth.b64(value['google_response_original_base64'],131072),self.transport.last_response)
        self.assertEqual(value['raw_verifier_original_sha256'],auth.sha(raw).hex())
        self.assertEqual((self.transport.revocations,self.transport.decodes),(1,1))
        self.assertEqual(self.owner.perform(request,'recover-finish'),result)
        self.assertEqual((self.transport.revocations,self.transport.decodes),(1,1))

    def test_recovery_returns_exact_original_after_expiry_without_new_capture(self):
        request,raw=self.raw_result()
        self.owner.current_time=self.now+180000
        with patch.object(self.owner,'raw',side_effect=AssertionError('recovery cannot verify')),patch('time.time',return_value=(self.now+180000)/1000):
            self.assertEqual(self.owner.perform(request,'recover-raw'),raw)
        self.assertEqual(self.owner.current_time,self.now+180000)
        changed=auth.exact_json(request,auth.MAX_ORIGINAL);changed['trusted_time_ms']=self.now+180000
        with self.assertRaisesRegex(AttestationRejected,'recovery_original'): self.owner.perform(auth.encode(changed),'recover-raw')
        self.assertEqual(self.transport.revocations,1)

    def test_finish_requires_retained_raw_original_not_caller_projection(self):
        request,raw=self.raw_result()
        offered=auth.exact_json(raw,auth.MAX_ORIGINAL);offered['checked_at_ms']+=1
        finish=self.finish_request(auth.encode(offered))
        with self.assertRaisesRegex(AttestationRejected,'raw_not_retained'): self.owner.perform(finish,'verify-finish')
        self.assertEqual(self.transport.decodes,0)

    def test_finish_cannot_predate_raw_verification(self):
        _,raw=self.raw_result()
        finish=self.finish_request(raw,now=self.now-1)
        with self.assertRaisesRegex(AttestationRejected,'raw_capture_time'): self.owner.perform(finish,'verify-finish')
        self.assertEqual(self.transport.decodes,0)

    def test_integrity_wrong_request_hash_rejects_and_retry_does_not_decode_again(self):
        _,raw=self.raw_result();request=self.finish_request(raw)
        self.transport.mutate=lambda value:value['requestDetails'].update(requestHash=pi.request_hash_text(b'\x55'*32))
        with self.assertRaises(AttestationRejected): self.owner.perform(request,'verify-finish')
        self.assertIsNone(self.owner.perform(request,'recover-finish'))
        with self.assertRaisesRegex(AttestationRejected,'attempt_consumed'): self.owner.perform(request,'verify-finish')
        self.assertEqual(self.transport.decodes,1)

    def test_integrity_testing_override_rejects(self):
        _,raw=self.raw_result();request=self.finish_request(raw)
        self.transport.mutate=lambda value:value.update(testingDetails={'isTestingResponse':True})
        with self.assertRaises(AttestationRejected): self.owner.perform(request,'verify-finish')

    def test_integrity_foreign_signer_or_software_verdict_rejects(self):
        _,raw=self.raw_result();request=self.finish_request(raw)
        self.transport.mutate=lambda value:value['deviceIntegrity'].update(deviceRecognitionVerdict=['MEETS_BASIC_INTEGRITY'])
        with self.assertRaises(AttestationRejected): self.owner.perform(request,'verify-finish')
        self.assertEqual(self.transport.decodes,1)

    def test_changed_verifier_config_cannot_recover_old_success(self):
        request,_=self.raw_result()
        self.owner.config_digest=b'\x12'*32
        with self.assertRaisesRegex(AttestationRejected,'recovery_binding'): self.owner.perform(request,'recover-raw')

    def test_request_extra_fields_wrong_phase_and_stale_capture_reject(self):
        request=auth.exact_json(self.raw_request(),auth.MAX_ORIGINAL)
        for changed in (dict(request,caller_verdict=True),dict(request,phase='finish'),dict(request,trusted_time_ms=True),dict(request,trusted_time_ms=self.now+180000)):
            with self.subTest(changed=list(changed)),self.assertRaises(AttestationRejected): self.owner.perform(auth.encode(changed),'verify-raw')
        self.assertEqual(self.transport.revocations,0)

    def test_reserved_attempt_without_result_is_unknown_and_never_reissued(self):
        request=self.raw_request();digest=auth.sha(self.transcript)
        self.journal.reserve('raw',digest,request)
        self.assertIsNone(self.owner.perform(request,'recover-raw'))
        with self.assertRaisesRegex(AttestationRejected,'attempt_consumed'): self.owner.perform(request,'verify-raw')
        self.assertEqual(self.transport.revocations,0)

    def test_journal_retention_cannot_replace_success(self):
        request,raw=self.raw_result()
        with self.assertRaisesRegex(AttestationRejected,'result_replaced'): self.journal.retain('raw',auth.sha(self.transcript),request,raw+b'X')
        self.assertEqual(self.journal.cached('raw',auth.sha(self.transcript),request),raw)

    def test_journal_rejects_replaced_database(self):
        self.journal.path.rename(self.directory/'old-database')
        self.journal.path.touch(mode=0o600)
        with self.assertRaisesRegex(AttestationRejected,'journal_replaced'): self.journal.connect()

    def test_journal_rejects_insecure_database_and_sidecar(self):
        self.journal.path.chmod(0o644)
        with self.assertRaisesRegex(AttestationRejected,'journal_custody'): self.journal.connect()
        self.journal.path.chmod(0o600)
        sidecar=self.journal.path.with_name(self.journal.path.name+'-journal');sidecar.touch(mode=0o644)
        with self.assertRaisesRegex(AttestationRejected,'journal_sidecar'): self.journal.connect()

    def test_journal_rejects_symlink_directory_or_database(self):
        self.journal.path.rename(self.directory/'old-database')
        self.journal.path.symlink_to(self.directory/'old-database')
        with self.assertRaisesRegex(AttestationRejected,'journal_custody'): self.journal.connect()

    def test_store_permissions_change_never_becomes_missing_attempt(self):
        self.directory.chmod(0o755)
        with self.assertRaisesRegex(AttestationRejected,'store_custody'):self.journal.cached('raw',auth.sha(self.transcript),self.raw_request())

    def test_journal_closed_phase_digest_and_original_bounds(self):
        for args in (('E1',b'\1'*32,b'x'),('raw',b'\0'*32,b'x'),('raw',b'\1'*31,b'x'),('raw',b'\1'*32,b'')):
            with self.subTest(args=args[:1]),self.assertRaises(AttestationRejected): self.journal.reserve(*args)

    def test_public_file_identity_recheck_detects_replacement_and_change(self):
        path=self.directory/'public';path.write_bytes(b'public original');path.chmod(0o600)
        fd=os.open(path,os.O_RDONLY)
        try:
            held=auth.HeldPublicFile(fd,auth.sha(b'public original'),path)
            path.write_bytes(b'changed public')
            with self.assertRaises(AttestationRejected): held.recheck()
        finally: os.close(fd)


class ProtocolTests(unittest.TestCase):
    def frame(self,value):
        packet=auth.encode(value);return len(packet).to_bytes(4,'little')+packet
    def request(self,exchange=1):
        return {'schema':auth.SCHEMA,'version':1,'exchange_id':(bytes([exchange])*32).hex(),
                'action':'recover-raw','original_base64':auth.text(b'synthetic original')}
    def test_fragmented_header_and_body_are_read_exactly(self):
        class Partial(io.BytesIO):
            def read(self,width=-1): return super().read(min(width,1))
        self.assertEqual(auth.read_packet(Partial((3).to_bytes(4,'little')+b'abc')),b'abc')
    def test_truncated_zero_or_large_frames_reject(self):
        for value in (b'\x01',b'\0'*4,(auth.MAX_PACKET+1).to_bytes(4,'little'),(2).to_bytes(4,'little')+b'x'):
            with self.subTest(value=value),self.assertRaises(AttestationRejected):auth.read_packet(io.BytesIO(value))
    def test_unknown_recovery_is_explicit_and_exchange_bound(self):
        class Owner:
            def perform(self,original,action):
                if original!=b'synthetic original' or action!='recover-raw': raise AssertionError('changed request')
                return None
        output=io.BytesIO();auth.serve(Owner(),io.BytesIO(self.frame(self.request())),output)
        response=auth.exact_json(auth.read_packet(io.BytesIO(output.getvalue())),auth.MAX_PACKET)
        self.assertEqual(response['outcome'],'outcome_unknown');self.assertIsNone(response['result_base64'])
        self.assertEqual(response['exchange_id'],self.request()['exchange_id'])
        self.assertEqual(response['original_sha256'],auth.sha(b'synthetic original').hex())
    def test_closed_error_responses_never_leak_bearer_or_der(self):
        for error,expected in ((AttestationRejected('secret bearer token'),'rejected'),
                               (VerificationUnavailable('secret bearer token'),'unavailable'),
                               (sqlite3.OperationalError('secret bearer token'),'unavailable')):
            class Owner:
                def perform(self,*unused):raise error
            output=io.BytesIO();auth.serve(Owner(),io.BytesIO(self.frame(self.request())),output)
            self.assertNotIn(b'secret',output.getvalue())
            response=auth.exact_json(auth.read_packet(io.BytesIO(output.getvalue())),auth.MAX_PACKET)
            self.assertEqual(response['outcome'],expected);self.assertIsNone(response['result_base64'])
    def test_duplicate_exchange_and_unknown_packet_fields_reject(self):
        class Owner:
            def perform(self,*unused):return None
        original=self.frame(self.request())
        with self.assertRaisesRegex(AttestationRejected,'exchange_reused'):auth.serve(Owner(),io.BytesIO(original+original),io.BytesIO())
        with self.assertRaisesRegex(AttestationRejected,'exchange_fields'):auth.serve(Owner(),io.BytesIO(self.frame(dict(self.request(),issuer_key='forbidden'))),io.BytesIO())
    def test_sustained_private_traffic_exceeds_window_without_retiring_worker(self):
        class Owner:
            calls=0
            def perform(self,original,action):
                if original!=b'synthetic original' or action!='recover-raw':raise AssertionError('changed original')
                self.calls+=1
                return None
        packets=[]
        for number in range(1,auth.EXCHANGE_WINDOW+3):
            request=self.request();request['exchange_id']=number.to_bytes(32,'little').hex()
            packets.append(self.frame(request))
        # An aged transport tag creates no fresh operation: this is still cached-only recovery.
        packets.append(packets[0])
        owner=Owner();output=io.BytesIO()
        auth.serve(owner,io.BytesIO(b''.join(packets)),output)
        responses=io.BytesIO(output.getvalue());count=0
        while (packet:=auth.read_packet(responses)) is not None:
            response=auth.exact_json(packet,auth.MAX_PACKET)
            self.assertEqual(response['outcome'],'outcome_unknown');self.assertIsNone(response['result_base64']);count+=1
        self.assertEqual(count,auth.EXCHANGE_WINDOW+3)
        self.assertEqual(owner.calls,count)
    def test_config_recheck_failure_has_no_success_result(self):
        class Owner:
            def perform(self,*unused):raise AssertionError('must not dispatch')
        def changed():raise AttestationRejected('configuration changed')
        output=io.BytesIO();auth.serve(Owner(),io.BytesIO(self.frame(self.request())),output,changed)
        result=auth.exact_json(auth.read_packet(io.BytesIO(output.getvalue())),auth.MAX_PACKET)
        self.assertEqual(result['outcome'],'rejected');self.assertIsNone(result['result_base64'])
    def test_archive_is_exact_auth_only_closure(self):
        archive=Path(os.environ['BPNG_AUTH_TEST_ARCHIVE'])
        with zipfile.ZipFile(archive) as content:
            self.assertEqual(set(content.namelist()),{'__main__.py','iroha_app_attestation/__init__.py',
                'iroha_app_attestation/attestation.py','iroha_app_attestation/revocation.py',
                'iroha_app_attestation/play_integrity.py','iroha_app_attestation/google_oauth.py',
                'iroha_app_attestation/openssl_private_rsa.py','iroha_app_attestation/native_time_interval.py',
                'iroha_app_attestation/retail_auth_worker.py'})
            self.assertFalse(any('wallet_enrollment' in name or 'apple_receipt' in name for name in content.namelist()))
            self.assertTrue(all(info.date_time==(1980,1,1,0,0,0) and info.compress_type==zipfile.ZIP_STORED for info in content.infolist()))
            self.assertEqual(content.read('iroha_app_attestation/retail_auth_worker.py'),Path(os.environ['BPNG_AUTH_TEST_WORKER']).read_bytes())


class ArchiveBuilderTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        path=Path(os.environ['BPNG_AUTH_TEST_BUILDER'])
        spec=importlib.util.spec_from_file_location('auth_archive_builder_under_test',path)
        cls.builder=importlib.util.module_from_spec(spec);spec.loader.exec_module(cls.builder)
    def setUp(self):
        self.temporary=tempfile.TemporaryDirectory(prefix='bpng-auth-archive-tests-')
        self.root=Path(self.temporary.name).resolve()
        self.generic=self.root/'generic';self.auth=self.root/'auth'
        package=self.generic/'src/iroha_app_attestation';package.mkdir(parents=True)
        auth_package=self.auth/'src/iroha_app_attestation';auth_package.mkdir(parents=True)
        # Explicit inert source originals exercise builder custody only.
        for name in self.builder.GENERIC:(package/name).write_text('# inert builder custody fixture\n')
        (auth_package/'retail_auth_worker.py').write_text('# inert auth builder fixture\n')
    def tearDown(self):self.temporary.cleanup()
    def output(self,name):
        directory=self.root/name;directory.mkdir()
        return directory/'iroha-retail-auth-verifier.pyz'
    def test_unsigned_build_is_deterministic_and_inventory_is_exact(self):
        first=self.builder.build(self.generic,self.auth,self.output('a'))
        second=self.builder.build(self.generic,self.auth,self.output('b'))
        self.assertEqual(first,second)
        self.assertFalse(first['signed_runtime_admission'])
        self.assertEqual(len(first['sources']),9)
        self.assertEqual((self.root/'a/iroha-retail-auth-verifier.pyz').stat().st_mode&0o777,0o444)
    def test_existing_output_is_preserved(self):
        output=self.output('a');output.write_bytes(b'preserved original')
        with self.assertRaises(FileExistsError):self.builder.build(self.generic,self.auth,output)
        self.assertEqual(output.read_bytes(),b'preserved original')
    def test_alias_source_is_rejected(self):
        alias=self.root/'generic-alias';alias.symlink_to(self.generic,target_is_directory=True)
        with self.assertRaisesRegex(ValueError,'source_alias'):self.builder.build(alias,self.auth,self.output('a'))
    def test_source_mutation_during_archive_build_is_refused(self):
        source=self.generic/'src/iroha_app_attestation/attestation.py'
        original=zipfile.ZipFile.writestr
        changed=False
        def mutation(archive,*args,**kwargs):
            nonlocal changed
            if not changed:source.write_bytes(source.read_bytes()+b'# changed during archive\n');changed=True
            return original(archive,*args,**kwargs)
        with patch.object(zipfile.ZipFile,'writestr',mutation):
            with self.assertRaisesRegex(ValueError,'source_changed'):self.builder.build(self.generic,self.auth,self.output('a'))
    def test_wrong_archive_name_is_not_created(self):
        output=self.root/'foreign.pyz'
        with self.assertRaisesRegex(ValueError,'archive_name'):self.builder.build(self.generic,self.auth,output)
        self.assertFalse(output.exists())


if __name__=='__main__': unittest.main()
