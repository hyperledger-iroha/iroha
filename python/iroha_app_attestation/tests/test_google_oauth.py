from iroha_app_attestation.native_time_interval import NativeTimeInterval
"""Actual synthetic RSA signing and mocked fixed Google OAuth transport.

No installed credential, Google endpoint or genuine verdict is used. An explicit
fixture replaces Root admission only for these primitive tests; actual public
code FDs, bytes and image selections are still observed. This does not qualify
the unapproved development Python/OpenSSL runtime or any Native issuer.
"""
import base64
import hashlib
import json
import os
import shutil
import subprocess
import tempfile
import unittest
import urllib.error
import urllib.parse
from dataclasses import replace
from pathlib import Path
from unittest.mock import patch

from iroha_app_attestation import openssl_private_rsa as crypto
from iroha_app_attestation.attestation import AttestationRejected, require
from iroha_app_attestation.google_oauth import (
    GoogleServiceAccountTokenProvider, MAX_TOKEN_RESPONSE_BYTES, OAUTH_SCOPE,
    POLICY_SCHEMA, TOKEN_URI, select_google_decoder,
    _key_command,
)
from iroha_app_attestation.play_integrity import PlayIntegrityPolicy, PlayIntegrityUnavailable

PROJECT = 'example-integrity-project'
EMAIL = 'integrity-decoder@' + PROJECT + '.iam.gserviceaccount.com'
CLIENT_ID = '116801097826214108702'
CERT = b'\x22' * 32


def public_original():
    return json.dumps({'schema': POLICY_SCHEMA, 'version': 1,
        'cloudProject': {'id': PROJECT, 'number': 642560099159},
        'packageName': 'org.example.app', 'packageVersion': 28,
        'appSigningCertificateSha256Hex': CERT.hex(),
        'credentialSubject': {'email': EMAIL, 'clientId': CLIENT_ID}},
        separators=(',', ':')).encode()


ORIGINAL = public_original()
POLICY = PlayIntegrityPolicy(hashlib.sha256(ORIGINAL).digest(), 'org.example.app', 28,
                            CERT, 120_000, 60_000, True, True, 'MEETS_DEVICE_INTEGRITY')


class Response:
    status = 200
    url = TOKEN_URI
    headers = {'Content-Type': 'application/json'}
    body = json.dumps({'access_token': 'synthetic-access-token', 'token_type': 'Bearer',
                       'expires_in': 3600, 'scope': OAUTH_SCOPE}).encode()
    def __enter__(self): return self
    def __exit__(self, *args): pass
    def geturl(self): return self.url
    def read(self, bound):
        self.bound = bound
        return self.body[:bound]


class SyntheticCodeOriginal:
    """Test-only public-code FD fixture; intentionally provides no Root admission."""
    _digest = crypto._HeldRootCodeOriginal._digest

    def __init__(self,path):
        self.path=path
        self.fd=os.open(path,os.O_RDONLY|os.O_NOFOLLOW|os.O_CLOEXEC)
        self.original=crypto._identity(os.fstat(self.fd))
        self.digest=self._digest()

    def recheck(self):
        require(self.fd>=0 and not os.get_inheritable(self.fd)
                and crypto._identity(os.fstat(self.fd))==self.original
                and crypto._identity(self.path.lstat())==self.original
                and self._digest()==self.digest,'synthetic TLS code original changed')

    def close(self):
        if self.fd>=0:os.close(self.fd);self.fd=-1


class GoogleOAuthTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.openssl = Path(shutil.which('openssl')).resolve()
        result = subprocess.run([str(cls.openssl), 'genpkey', '-algorithm', 'RSA',
                                 '-pkeyopt', 'rsa_keygen_bits:2048'],
                                capture_output=True, check=True)
        cls.pem = result.stdout.decode('ascii')

    def setUp(self):
        # Production has no bypass. Only this synthetic primitive suite replaces
        # Root ownership admission, never policy/credential/image verification.
        self.code_fixture=patch.object(crypto,'_HeldRootCodeOriginal',SyntheticCodeOriginal)
        self.code_fixture.start()
        self.addCleanup(self.code_fixture.stop)
        self.temporary = tempfile.TemporaryDirectory()
        self.path = Path(self.temporary.name) / 'synthetic-only.json'
        self.now = 1_800_000_000_000
        self.providers = []
        self.write_credential()
        self.fd = os.open(self.path, os.O_RDONLY)

    def tearDown(self):
        for provider in self.providers: provider.close()
        os.close(self.fd)
        self.temporary.cleanup()

    def credential(self):
        return {'type': 'service_account', 'project_id': PROJECT,
            'private_key_id': 'ab' * 20, 'private_key': self.pem,
            'client_email': EMAIL, 'client_id': CLIENT_ID,
            'auth_uri': 'https://accounts.google.com/o/oauth2/auth', 'token_uri': TOKEN_URI,
            'auth_provider_x509_cert_url': 'https://www.googleapis.com/oauth2/v1/certs',
            'client_x509_cert_url': 'https://www.googleapis.com/robot/v1/metadata/x509/'
                + urllib.parse.quote(EMAIL, safe=''), 'universe_domain': 'googleapis.com'}

    def write_credential(self, **changes):
        value = self.credential(); value.update(changes)
        self.path.write_text(json.dumps(value)); self.path.chmod(0o600)

    def provider(self, **changes):
        inputs = {'public_policy_original': ORIGINAL, 'native_policy': POLICY,
                  'credential_fd': self.fd, 'trusted_time_interval': lambda: NativeTimeInterval(self.now,self.now),
                  'openssl_path': self.openssl}; inputs.update(changes)
        value = GoogleServiceAccountTokenProvider(**inputs)
        self.providers.append(value)
        return value

    def test_native_pin_exact_identity_and_public_layout_are_required(self):
        selected = select_google_decoder(ORIGINAL, POLICY)
        self.assertEqual(selected.service_account_email, EMAIL)
        for original in (ORIGINAL + b' ', b'{}', ORIGINAL.replace(b'28', b'27', 1)):
            with self.subTest(original=original[:40]), self.assertRaises(AttestationRejected):
                select_google_decoder(original, POLICY)
        for field, value in (('packageVersion', True), ('packageVersion', 27),
            ('appSigningCertificateSha256Hex', '00'*32), ('credentialSubject', {'email': EMAIL, 'clientId': 123}),
            ('cloudProject', {'id': PROJECT, 'number': True}), ('unexpected', 'value')):
            parsed = json.loads(ORIGINAL); parsed[field] = value
            original = json.dumps(parsed).encode()
            with self.subTest(field=field), self.assertRaises(AttestationRejected):
                select_google_decoder(original, replace(POLICY, policy_digest=hashlib.sha256(original).digest()))
        original = ORIGINAL[:-1] + b',"version":1}'
        with self.assertRaises(AttestationRejected):
            select_google_decoder(original, replace(POLICY, policy_digest=hashlib.sha256(original).digest()))

    def test_invalid_public_policy_never_opens_private_custody(self):
        for original in (None, {}, "untrusted", b"", bytearray(ORIGINAL), b" " * (16*1024+1)):
            with self.subTest(kind=type(original).__name__), patch('iroha_app_attestation.google_oauth.os.dup') as duplicate:
                with self.assertRaises(AttestationRejected): self.provider(public_policy_original=original)
                duplicate.assert_not_called()

    def test_credential_project_principal_urls_and_ownership_fail_closed(self):
        for change in ({'project_id': 'other-project'}, {'client_email': 'other@' + PROJECT + '.iam.gserviceaccount.com'},
            {'client_id': '999'}, {'type': 'authorized_user'}, {'token_uri': 'https://attacker.example/token'},
            {'auth_uri': 'https://attacker.example'}, {'universe_domain': 'attacker.example'},
            {'private_key_id': 'invalid'}, {'private_key': self.pem + 'trailing'},
            {'client_x509_cert_url': 'https://attacker.example/key'}):
            self.write_credential(**change)
            with self.subTest(change=list(change)), self.assertRaises(AttestationRejected): self.provider()
        self.write_credential(); self.path.chmod(0o644)
        with self.assertRaises(AttestationRejected): self.provider()
        read_fd, write_fd = os.pipe()
        try:
            with self.assertRaises(AttestationRejected): self.provider(credential_fd=read_fd)
        finally:
            os.close(read_fd); os.close(write_fd)

    def test_real_rs256_assertion_has_only_fixed_decoder_scope_and_no_impersonation(self):
        os.lseek(self.fd, 7, os.SEEK_SET)
        provider = self.provider(); response = Response()
        with patch('iroha_app_attestation.google_oauth.urllib.request.build_opener') as build:
            build.return_value.open.return_value = response
            self.assertEqual(provider(), 'synthetic-access-token')
            request = build.return_value.open.call_args.args[0]
            self.assertEqual(request.full_url, TOKEN_URI)
            self.assertEqual(request.get_method(), 'POST')
            self.assertEqual(build.return_value.open.call_args.kwargs, {'timeout': 5})
            self.assertEqual(build.call_args.args[0].proxies, {})
            form = urllib.parse.parse_qs(request.data.decode(), strict_parsing=True)
            self.assertEqual(set(form), {'grant_type', 'assertion'})
            self.assertEqual(form['grant_type'], ['urn:ietf:params:oauth:grant-type:jwt-bearer'])
            header, claims, signature = form['assertion'][0].split('.')
            decode = lambda value: base64.urlsafe_b64decode(value + '=' * (-len(value) % 4))
            self.assertEqual(json.loads(decode(header)), {'alg': 'RS256', 'typ': 'JWT', 'kid': 'ab'*20})
            self.assertEqual(json.loads(decode(claims)), {'iss': EMAIL, 'scope': OAUTH_SCOPE,
                'aud': TOKEN_URI, 'iat': self.now//1000, 'exp': self.now//1000 + 300})
            # Independent actual OpenSSL verification of the generated public
            # JWT. Only the synthetic fixture key touches this test directory.
            private = Path(self.temporary.name) / 'fixture.pem'; private.write_text(self.pem); private.chmod(0o600)
            public = Path(self.temporary.name) / 'fixture.pub'
            subprocess.run([str(self.openssl), 'pkey', '-in', str(private), '-pubout', '-out', str(public)],
                           capture_output=True, check=True)
            sig = Path(self.temporary.name) / 'assertion.sig'; sig.write_bytes(decode(signature))
            result = subprocess.run([str(self.openssl), 'dgst', '-sha256', '-verify', str(public),
                '-signature', str(sig)], input=(header + '.' + claims).encode(), capture_output=True)
            self.assertEqual(result.returncode, 0)
            self.assertEqual(response.bound, MAX_TOKEN_RESPONSE_BYTES + 1)
        self.assertEqual(os.lseek(self.fd, 0, os.SEEK_CUR), 7)

    def test_token_cache_expiry_clock_rollback_and_closed_custody(self):
        provider = self.provider()
        with patch('iroha_app_attestation.google_oauth.urllib.request.build_opener') as build:
            build.return_value.open.return_value = Response()
            provider(); self.now += 1000; provider()
            self.assertEqual(build.return_value.open.call_count, 1)
            self.now += 3_540_000; provider()
            self.assertEqual(build.return_value.open.call_count, 2)
            self.now -= 1000; provider()
            self.assertEqual(build.return_value.open.call_count, 3)
        provider.close()
        with self.assertRaises(AttestationRejected): provider()

    def test_upper_bound_expiry_refuses_cached_access_without_future_iat(self):
        provider=self.provider()
        with patch('iroha_app_attestation.google_oauth.urllib.request.build_opener') as build:
            build.return_value.open.return_value=Response();provider()
            original_lower=self.now
            # Lower remains valid but the upper bound crosses the original refresh deadline.
            provider._clock=lambda:NativeTimeInterval(original_lower,original_lower+3_540_000)
            with self.assertRaisesRegex(AttestationRejected,'trusted time changed'):provider()
            self.assertIsNone(provider._access)
            self.assertEqual(build.return_value.open.call_count,2)
            request=build.return_value.open.call_args.args[0]
            assertion=urllib.parse.parse_qs(request.data.decode())['assertion'][0]
            claims=assertion.split('.')[1]
            parsed=json.loads(base64.urlsafe_b64decode(claims+'='*(-len(claims)%4)))
            self.assertEqual(parsed['iat'],original_lower//1000)

    def test_actual_dependency_image_substitutions_fail_closed(self):
        # Actual code/image data with the explicit test-only Root fixture above.
        # These private holders are observations, never Native capabilities.
        held=crypto.acquire_crypto_originals()
        original_crypto,original_tls=held.crypto_path,held.tls_path
        try:
            held.recheck()
            held.crypto_path=original_tls
            with self.assertRaisesRegex(AttestationRejected,'dependency selection changed'):held.recheck()
            held.crypto_path=original_crypto;held.tls_path=original_crypto
            with self.assertRaisesRegex(AttestationRejected,'dependency selection changed'):held.recheck()
            held.tls_path=original_tls
            real_function=held.library.BIO_new_mem_buf
            held.library.BIO_new_mem_buf=held.library.SSL_CTX_new
            with self.assertRaisesRegex(AttestationRejected,'dependency selection changed'):held.recheck()
            held.library.BIO_new_mem_buf=real_function
            held.recheck()
        finally:held.close()

    def test_changed_held_code_blocks_before_cached_token_or_credential_read(self):
        provider=self.provider()
        with patch('iroha_app_attestation.google_oauth.urllib.request.build_opener') as build:
            build.return_value.open.return_value=Response();provider()
            provider._crypto_code.originals[0].digest=b'\xff'*32
            with patch.object(provider,'_read') as read, self.assertRaisesRegex(
                    AttestationRejected,'synthetic TLS code original changed'):
                provider()
            read.assert_not_called()
            self.assertEqual(build.return_value.open.call_count,1)

    def test_changed_code_during_exchange_cannot_publish_a_token(self):
        provider=self.provider()
        response=Response()
        original_read=response.read
        def change_code(bound):
            provider._crypto_code.originals[0].digest=b'\xff'*32
            return original_read(bound)
        response.read=change_code
        with patch('iroha_app_attestation.google_oauth.urllib.request.build_opener') as build:
            build.return_value.open.return_value=response
            with self.assertRaisesRegex(AttestationRejected,'synthetic TLS code original changed'):
                provider()
            self.assertIsNone(provider._access)

    def test_changed_held_credential_blocks_even_cached_token(self):
        provider = self.provider()
        with patch('iroha_app_attestation.google_oauth.urllib.request.build_opener') as build:
            build.return_value.open.return_value = Response(); provider()
            self.write_credential(client_id='999')
            with self.assertRaisesRegex(AttestationRejected, 'original changed'): provider()
            self.assertEqual(build.return_value.open.call_count, 1)

    def test_response_failure_cannot_relax_transport_scope_or_token_bounds(self):
        for change in ({'status': 302}, {'url': 'https://attacker.example/token'},
            {'headers': {'Content-Type': 'text/html'}},
            {'headers': {'Content-Type': 'application/json', 'Content-Encoding': 'gzip'}},
            {'body': b' ' * (MAX_TOKEN_RESPONSE_BYTES+1)},
            {'body': b'{"access_token":"a","access_token":"b"}'},
            {'body': json.dumps({'access_token': 'a', 'token_type': 'Bearer', 'expires_in': True}).encode()},
            {'body': json.dumps({'access_token': 'a', 'token_type': 'Bearer', 'expires_in': 3600,
                                'scope': OAUTH_SCOPE + ' https://www.googleapis.com/auth/cloud-platform'}).encode()}):
            provider = self.provider(); response = Response()
            for field, value in change.items(): setattr(response, field, value)
            with patch('iroha_app_attestation.google_oauth.urllib.request.build_opener') as build:
                build.return_value.open.return_value = response
                # A changed or malformed Google answer is retryable, never a
                # verdict on the mobile evidence; it still fails closed.
                with self.subTest(change=list(change)), self.assertRaises(PlayIntegrityUnavailable): provider()
                self.assertIsNone(provider._access)
        for failure in (OSError('synthetic private content must not escape'),
                        urllib.error.HTTPError(TOKEN_URI, 400, 'invalid_grant', {}, None),
                        urllib.error.HTTPError(TOKEN_URI, 503, 'unavailable', {}, None)):
            provider = self.provider()
            with patch('iroha_app_attestation.google_oauth.urllib.request.build_opener') as build:
                build.return_value.open.side_effect = failure
                with self.subTest(failure=type(failure).__name__), self.assertRaisesRegex(
                        PlayIntegrityUnavailable, '^Google OAuth token exchange unavailable$') as raised:
                    provider()
                self.assertIsNone(raised.exception.__cause__)

    def test_ec_or_invalid_private_key_cannot_sign_rs256(self):
        result = subprocess.run([str(self.openssl), 'genpkey', '-algorithm', 'EC',
            '-pkeyopt', 'ec_paramgen_curve:P-256'], capture_output=True, check=True)
        self.write_credential(private_key=result.stdout.decode())
        with self.assertRaisesRegex(AttestationRejected, 'must be RSA'): self.provider()
        self.write_credential(private_key='-----BEGIN PRIVATE KEY-----\ninvalid\n-----END PRIVATE KEY-----\n')
        with self.assertRaisesRegex(AttestationRejected, '^Google OAuth signing operation failed$'): self.provider()

    def test_private_rsa_signing_does_not_spawn_or_publish_a_key_file(self):
        message=b'exact bounded OAuth signing fixture'
        with patch('subprocess.Popen',side_effect=AssertionError('private signing must not exec')):
            public=_key_command(self.pem.encode(),self.openssl,['pkey','-pubout','-outform','DER','-in'])
            first=_key_command(self.pem.encode(),self.openssl,['dgst','-sha256','-sign'],message)
            second=_key_command(self.pem.encode(),self.openssl,['dgst','-sha256','-sign'],message)
        self.assertTrue(public.startswith(b'0'))
        self.assertEqual(len(first),256)
        self.assertEqual(first,second)
        self.assertNotIn(b'PRIVATE KEY',public+first)
        for arguments,body in ((['dgst','-sha1','-sign'],message),
                               (['dgst','-sha256','-sign'],b''),
                               (['dgst','-sha256','-sign'],b'x'*4097),
                               (['pkey','-pubout','-outform','DER','-in'],message)):
            with self.subTest(arguments=arguments),self.assertRaises(AttestationRejected):
                _key_command(self.pem.encode(),self.openssl,arguments,body)


if __name__ == '__main__': unittest.main()
